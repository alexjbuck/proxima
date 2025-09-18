//! Spatial data structures and caching for Proxima
//!
//! This module implements efficient spatial data structures for geographic indexing,
//! content caching, and spatial queries.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};
use serde::{Deserialize, Serialize};
use spade::{RTree, SpatialObject, Point2, BoundingBox};
use dashmap::DashMap;
use lru::LruCache;
use thiserror::Error;

use crate::geo::*;
use crate::content::*;

/// A spatial object that can be stored in R-trees
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpatialContent {
    pub content_id: ContentId,
    pub location: GeographicLocation,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub relevance_score: f64,
}

impl SpatialObject for SpatialContent {
    type Point = Point2<f64>;
    
    fn mbr(&self) -> BoundingBox<Point2<f64>> {
        let (lat, lon) = self.location.coordinates;
        let uncertainty = self.location.uncertainty_meters / 111_000.0; // Rough conversion to degrees
        
        BoundingBox::from_corners(
            Point2::new(lon - uncertainty, lat - uncertainty),
            Point2::new(lon + uncertainty, lat + uncertainty),
        )
    }
    
    fn distance2(&self, point: &Point2<f64>) -> f64 {
        let (lat, lon) = self.location.coordinates;
        let dx = lon - point.x;
        let dy = lat - point.y;
        dx * dx + dy * dy
    }
}

/// A quadtree node for hierarchical spatial indexing
#[derive(Debug, Clone)]
pub struct QuadTreeNode {
    pub bounds: (f64, f64, f64, f64), // (min_lat, min_lon, max_lat, max_lon)
    pub content: Vec<SpatialContent>,
    pub children: Option<[Box<QuadTreeNode>; 4]>,
    pub max_content: usize,
    pub max_depth: usize,
    pub current_depth: usize,
}

impl QuadTreeNode {
    /// Create a new quadtree node
    pub fn new(
        bounds: (f64, f64, f64, f64),
        max_content: usize,
        max_depth: usize,
        current_depth: usize,
    ) -> Self {
        Self {
            bounds,
            content: Vec::new(),
            children: None,
            max_content,
            max_depth,
            current_depth,
        }
    }
    
    /// Insert content into the quadtree
    pub fn insert(&mut self, spatial_content: SpatialContent) {
        if !self.contains(&spatial_content.location) {
            return;
        }
        
        if self.children.is_none() && self.content.len() < self.max_content {
            self.content.push(spatial_content);
            return;
        }
        
        if self.children.is_none() && self.current_depth < self.max_depth {
            self.subdivide();
        }
        
        if let Some(ref mut children) = self.children {
            for child in children.iter_mut() {
                if child.contains(&spatial_content.location) {
                    child.insert(spatial_content);
                    return;
                }
            }
        }
        
        // If we can't subdivide further, just add to this node
        self.content.push(spatial_content);
    }
    
    /// Query content within a geographic region
    pub fn query_region(&self, bounds: (f64, f64, f64, f64)) -> Vec<&SpatialContent> {
        let mut results = Vec::new();
        
        if !self.bounds_intersect(bounds) {
            return results;
        }
        
        // Add content from this node
        for content in &self.content {
            if self.point_in_bounds(content.location.coordinates, bounds) {
                results.push(content);
            }
        }
        
        // Query children
        if let Some(ref children) = self.children {
            for child in children.iter() {
                results.extend(child.query_region(bounds));
            }
        }
        
        results
    }
    
    /// Query content within a radius of a point
    pub fn query_radius(&self, center: GeographicLocation, radius_meters: f64) -> Vec<&SpatialContent> {
        let mut results = Vec::new();
        
        // Convert radius to approximate degrees (rough approximation)
        let radius_degrees = radius_meters / 111_000.0;
        let (center_lat, center_lon) = center.coordinates;
        
        let bounds = (
            center_lat - radius_degrees,
            center_lon - radius_degrees,
            center_lat + radius_degrees,
            center_lon + radius_degrees,
        );
        
        let candidates = self.query_region(bounds);
        
        for content in candidates {
            let distance = center.distance_to(&content.location);
            if distance <= radius_meters {
                results.push(content);
            }
        }
        
        results
    }
    
    /// Check if a location is within this node's bounds
    fn contains(&self, location: &GeographicLocation) -> bool {
        self.point_in_bounds(location.coordinates, self.bounds)
    }
    
    /// Check if a point is within bounds
    fn point_in_bounds(&self, (lat, lon): (f64, f64), bounds: (f64, f64, f64, f64)) -> bool {
        let (min_lat, min_lon, max_lat, max_lon) = bounds;
        lat >= min_lat && lat <= max_lat && lon >= min_lon && lon <= max_lon
    }
    
    /// Check if two bounding boxes intersect
    fn bounds_intersect(&self, other_bounds: (f64, f64, f64, f64)) -> bool {
        let (min_lat1, min_lon1, max_lat1, max_lon1) = self.bounds;
        let (min_lat2, min_lon2, max_lat2, max_lon2) = other_bounds;
        
        !(max_lat1 < min_lat2 || min_lat1 > max_lat2 || max_lon1 < min_lon2 || min_lon1 > max_lon2)
    }
    
    /// Subdivide this node into four children
    fn subdivide(&mut self) {
        let (min_lat, min_lon, max_lat, max_lon) = self.bounds;
        let mid_lat = (min_lat + max_lat) / 2.0;
        let mid_lon = (min_lon + max_lon) / 2.0;
        
        let children = [
            Box::new(QuadTreeNode::new(
                (min_lat, min_lon, mid_lat, mid_lon),
                self.max_content,
                self.max_depth,
                self.current_depth + 1,
            )),
            Box::new(QuadTreeNode::new(
                (min_lat, mid_lon, mid_lat, max_lon),
                self.max_content,
                self.max_depth,
                self.current_depth + 1,
            )),
            Box::new(QuadTreeNode::new(
                (mid_lat, min_lon, max_lat, mid_lon),
                self.max_content,
                self.max_depth,
                self.current_depth + 1,
            )),
            Box::new(QuadTreeNode::new(
                (mid_lat, mid_lon, max_lat, max_lon),
                self.max_content,
                self.max_depth,
                self.current_depth + 1,
            )),
        ];
        
        // Move existing content to appropriate children
        let content_to_redistribute = std::mem::take(&mut self.content);
        self.children = Some(children);
        
        for content in content_to_redistribute {
            self.insert(content);
        }
    }
}

/// Geographic content cache with spatial indexing
pub struct GeographicContentCache {
    /// Quadtree for spatial indexing
    quadtree: Arc<dashmap::DashMap<String, QuadTreeNode>>,
    /// LRU cache for frequently accessed content
    lru_cache: Arc<dashmap::DashMap<ContentId, CachedContent>>,
    /// R-tree for efficient spatial queries
    rtree: Arc<dashmap::DashMap<String, RTree<SpatialContent>>>,
    /// Geographic bloom filters for different regions
    bloom_filters: Arc<dashmap::DashMap<String, GeographicBloomFilter>>,
    /// Cache configuration
    config: CacheConfig,
}

/// Cached content with metadata
#[derive(Debug, Clone)]
pub struct CachedContent {
    pub content: Content,
    pub spatial_content: SpatialContent,
    pub access_count: u64,
    pub last_accessed: Instant,
    pub insertion_time: Instant,
}

/// Geographic bloom filter for efficient content presence queries
#[derive(Debug, Clone)]
pub struct GeographicBloomFilter {
    pub region: GeographicRegion,
    pub filter: bloomfilter::Bloom,
    pub content_count: usize,
    pub last_updated: chrono::DateTime<chrono::Utc>,
}

/// Cache configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    pub max_content_per_quadtree: usize,
    pub max_quadtree_depth: usize,
    pub lru_cache_size: usize,
    pub content_ttl: Duration,
    pub bloom_filter_size: usize,
    pub bloom_filter_hash_count: usize,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            max_content_per_quadtree: 100,
            max_quadtree_depth: 10,
            lru_cache_size: 10000,
            content_ttl: Duration::from_secs(3600), // 1 hour
            bloom_filter_size: 10000,
            bloom_filter_hash_count: 7,
        }
    }
}

impl GeographicContentCache {
    /// Create a new geographic content cache
    pub fn new(config: CacheConfig) -> Self {
        Self {
            quadtree: Arc::new(dashmap::DashMap::new()),
            lru_cache: Arc::new(dashmap::DashMap::with_capacity(config.lru_cache_size)),
            rtree: Arc::new(dashmap::DashMap::new()),
            bloom_filters: Arc::new(dashmap::DashMap::new()),
            config,
        }
    }
    
    /// Insert content into the cache
    pub fn insert(&self, content: Content, location: GeographicLocation) -> Result<(), CacheError> {
        let content_id = content.id.clone();
        let spatial_content = SpatialContent {
            content_id: content_id.clone(),
            location: location.clone(),
            timestamp: chrono::Utc::now(),
            relevance_score: 1.0, // Will be calculated later
        };
        
        let cached_content = CachedContent {
            content: content.clone(),
            spatial_content: spatial_content.clone(),
            access_count: 0,
            last_accessed: Instant::now(),
            insertion_time: Instant::now(),
        };
        
        // Insert into LRU cache
        self.lru_cache.insert(content_id.clone(), cached_content);
        
        // Insert into spatial indexes
        self.insert_into_spatial_indexes(spatial_content)?;
        
        // Update bloom filters
        self.update_bloom_filters(&content_id, &location)?;
        
        Ok(())
    }
    
    /// Query content within a geographic region
    pub fn query_region(&self, bounds: (f64, f64, f64, f64)) -> Vec<Content> {
        let mut results = Vec::new();
        
        // Query from quadtree
        for quadtree in self.quadtree.iter() {
            let spatial_contents = quadtree.query_region(bounds);
            for spatial_content in spatial_contents {
                if let Some(cached) = self.lru_cache.get(&spatial_content.content_id) {
                    results.push(cached.content.clone());
                }
            }
        }
        
        results
    }
    
    /// Query content within a radius of a location
    pub fn query_radius(&self, center: GeographicLocation, radius_meters: f64) -> Vec<Content> {
        let mut results = Vec::new();
        
        // Query from quadtree
        for quadtree in self.quadtree.iter() {
            let spatial_contents = quadtree.query_radius(center.clone(), radius_meters);
            for spatial_content in spatial_contents {
                if let Some(cached) = self.lru_cache.get(&spatial_content.content_id) {
                    results.push(cached.content.clone());
                }
            }
        }
        
        results
    }
    
    /// Get content by ID
    pub fn get(&self, content_id: &ContentId) -> Option<Content> {
        if let Some(mut cached) = self.lru_cache.get_mut(content_id) {
            cached.access_count += 1;
            cached.last_accessed = Instant::now();
            Some(cached.content.clone())
        } else {
            None
        }
    }
    
    /// Check if content exists in a geographic region using bloom filter
    pub fn might_contain_in_region(&self, content_id: &ContentId, region: &GeographicRegion) -> bool {
        let region_key = format!("{:.6}_{:.6}_{:.6}_{:.6}", 
                                region.bounds.0, region.bounds.1, region.bounds.2, region.bounds.3);
        
        if let Some(bloom_filter) = self.bloom_filters.get(&region_key) {
            bloom_filter.filter.contains(content_id.as_bytes())
        } else {
            false
        }
    }
    
    /// Clean up expired content
    pub fn cleanup_expired(&self) {
        let now = Instant::now();
        let ttl = self.config.content_ttl;
        
        // Remove expired content from LRU cache
        self.lru_cache.retain(|_, cached| {
            now.duration_since(cached.insertion_time) < ttl
        });
        
        // Clean up empty bloom filters
        self.bloom_filters.retain(|_, bloom_filter| {
            bloom_filter.content_count > 0
        });
    }
    
    /// Get cache statistics
    pub fn stats(&self) -> CacheStats {
        CacheStats {
            lru_cache_size: self.lru_cache.len(),
            quadtree_count: self.quadtree.len(),
            bloom_filter_count: self.bloom_filters.len(),
            total_content: self.lru_cache.len(),
        }
    }
    
    /// Insert content into spatial indexes
    fn insert_into_spatial_indexes(&self, spatial_content: SpatialContent) -> Result<(), CacheError> {
        // Determine which quadtree to use based on geographic layer
        let layer = spatial_content.location.layer_for_distance(1000.0); // Default to neighborhood
        let quadtree_key = format!("{:?}", layer);
        
        // Get or create quadtree for this layer
        let mut quadtree = self.quadtree.entry(quadtree_key.clone()).or_insert_with(|| {
            QuadTreeNode::new(
                (-90.0, -180.0, 90.0, 180.0), // Global bounds
                self.config.max_content_per_quadtree,
                self.config.max_quadtree_depth,
                0,
            )
        });
        
        quadtree.insert(spatial_content.clone());
        
        // Also insert into R-tree
        let mut rtree = self.rtree.entry(quadtree_key).or_insert_with(|| RTree::new());
        rtree.insert(spatial_content);
        
        Ok(())
    }
    
    /// Update bloom filters for content
    fn update_bloom_filters(&self, content_id: &ContentId, location: &GeographicLocation) -> Result<(), CacheError> {
        // Update bloom filters for different geographic regions
        for layer in GeographicLayer::all_layers() {
            if let Some(address) = location.address_for_layer(layer) {
                let region_key = format!("{:?}_{}", layer, address.geohash);
                
                let mut bloom_filter = self.bloom_filters.entry(region_key).or_insert_with(|| {
                    GeographicBloomFilter {
                        region: GeographicRegion {
                            bounds: (-90.0, -180.0, 90.0, 180.0), // Will be updated
                            activity_level: 0.0,
                            content_count: 0,
                        },
                        filter: bloomfilter::Bloom::new(
                            self.config.bloom_filter_size,
                            self.config.bloom_filter_hash_count,
                        ),
                        content_count: 0,
                        last_updated: chrono::Utc::now(),
                    }
                });
                
                bloom_filter.filter.set(content_id.as_bytes());
                bloom_filter.content_count += 1;
                bloom_filter.last_updated = chrono::Utc::now();
            }
        }
        
        Ok(())
    }
}

/// Cache statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheStats {
    pub lru_cache_size: usize,
    pub quadtree_count: usize,
    pub bloom_filter_count: usize,
    pub total_content: usize,
}

/// Cache errors
#[derive(Error, Debug)]
pub enum CacheError {
    #[error("Cache insertion failed: {0}")]
    InsertionFailed(String),
    
    #[error("Spatial index error: {0}")]
    SpatialIndexError(String),
    
    #[error("Bloom filter error: {0}")]
    BloomFilterError(String),
    
    #[error("Cache configuration error: {0}")]
    ConfigError(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_quadtree_insertion_and_query() {
        let mut quadtree = QuadTreeNode::new(
            (-90.0, -180.0, 90.0, 180.0),
            10,
            5,
            0,
        );
        
        let location = GeographicLocation::new(37.7749, -122.4194, 10.0).unwrap();
        let spatial_content = SpatialContent {
            content_id: ContentId::new(),
            location,
            timestamp: chrono::Utc::now(),
            relevance_score: 1.0,
        };
        
        quadtree.insert(spatial_content);
        
        let results = quadtree.query_radius(
            GeographicLocation::new(37.7750, -122.4195, 10.0).unwrap(),
            1000.0,
        );
        
        assert_eq!(results.len(), 1);
    }
    
    #[test]
    fn test_geographic_cache_operations() {
        let cache = GeographicContentCache::new(CacheConfig::default());
        
        let location = GeographicLocation::new(37.7749, -122.4194, 10.0).unwrap();
        let content = Content {
            id: ContentId::new(),
            author: "test".to_string(),
            content_type: ContentType::Text,
            data: "Hello, world!".as_bytes().to_vec(),
            timestamp: chrono::Utc::now(),
            location: location.clone(),
            tags: vec![],
        };
        
        cache.insert(content.clone(), location).unwrap();
        
        let retrieved = cache.get(&content.id);
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().id, content.id);
    }
}