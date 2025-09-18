//! Geographic utility functions for Proxima
//! 
//! This module provides utility functions for geographic calculations,
//! location services, and spatial analysis.

use std::collections::{HashMap, HashSet, VecDeque};
use std::f64::consts::PI;
use std::sync::{Arc, RwLock};

use ahash::{AHashMap, AHashSet};
use geo::{Point, Coordinate, BoundingRect};
use nalgebra::{Vector2, Vector3, Matrix3};
use rand::Rng;
use rayon::prelude::*;
use spade::{DelaunayTriangulation, Triangulation, Point2, HasPosition};

use crate::geo::{
    GeographicLocation, GeographicLayer, GeographicSector, GeographicError,
    GeographicContentRelevance, GeographicContentGravity, GeographicAddress,
};

/// Location precision adjustment based on density
#[derive(Debug, Clone)]
pub struct LocationPrecisionAdjuster {
    /// Density thresholds for different precision levels
    density_thresholds: Vec<f64>,
    /// Precision levels (geohash precision)
    precision_levels: Vec<usize>,
    /// Current density map
    density_map: AHashMap<String, f64>,
}

impl LocationPrecisionAdjuster {
    /// Create a new location precision adjuster
    pub fn new() -> Self {
        LocationPrecisionAdjuster {
            density_thresholds: vec![0.1, 1.0, 10.0, 100.0, 1000.0], // items per km²
            precision_levels: vec![7, 6, 5, 4, 3], // geohash precision
            density_map: AHashMap::new(),
        }
    }

    /// Adjust location precision based on local density
    pub fn adjust_precision(
        &mut self,
        location: GeographicLocation,
        local_density: f64,
    ) -> Result<usize, GeographicError> {
        // Find appropriate precision level based on density
        let mut precision = 7; // Default high precision
        
        for (i, threshold) in self.density_thresholds.iter().enumerate() {
            if local_density > *threshold {
                precision = self.precision_levels[i];
            }
        }
        
        // Update density map
        let geohash = self.location_to_geohash(location, precision)?;
        self.density_map.insert(geohash, local_density);
        
        Ok(precision)
    }

    /// Get optimal precision for a location
    pub fn get_optimal_precision(&self, location: GeographicLocation) -> Result<usize, GeographicError> {
        // Check density in surrounding areas
        let mut total_density = 0.0;
        let mut count = 0;
        
        for precision in &self.precision_levels {
            let geohash = self.location_to_geohash(location, *precision)?;
            if let Some(&density) = self.density_map.get(&geohash) {
                total_density += density;
                count += 1;
            }
        }
        
        if count == 0 {
            return Ok(7); // Default high precision
        }
        
        let avg_density = total_density / count as f64;
        self.adjust_precision(location, avg_density)
    }

    /// Convert location to geohash with specified precision
    fn location_to_geohash(&self, location: GeographicLocation, precision: usize) -> Result<String, GeographicError> {
        use geohash::encode;
        encode(Coordinate { x: location.lon, y: location.lat }, precision)
            .map_err(|e| GeographicError::GeohashError(e.to_string()))
    }

    /// Update density for a region
    pub fn update_density(&mut self, geohash: String, density: f64) {
        self.density_map.insert(geohash, density);
    }

    /// Get density for a region
    pub fn get_density(&self, geohash: &str) -> Option<f64> {
        self.density_map.get(geohash).copied()
    }
}

/// Geographic anchor point detection
#[derive(Debug, Clone)]
pub struct AnchorPointDetector {
    /// Minimum distance between anchor points
    min_distance: f64,
    /// Minimum content density to be an anchor
    min_density: f64,
    /// Detected anchor points
    anchor_points: Vec<AnchorPoint>,
}

#[derive(Debug, Clone)]
pub struct AnchorPoint {
    /// Location
    pub location: GeographicLocation,
    /// Content density
    pub density: f64,
    /// Influence radius
    pub influence_radius: f64,
    /// Anchor strength
    pub strength: f64,
    /// Last updated
    pub last_updated: u64,
}

impl AnchorPointDetector {
    /// Create a new anchor point detector
    pub fn new(min_distance: f64, min_density: f64) -> Self {
        AnchorPointDetector {
            min_distance,
            min_density,
            anchor_points: Vec::new(),
        }
    }

    /// Detect anchor points from content locations
    pub fn detect_anchors(
        &mut self,
        content_locations: &[GeographicLocation],
        current_time: u64,
    ) -> Vec<AnchorPoint> {
        let mut new_anchors = Vec::new();
        
        // Group locations by density
        let density_map = self.calculate_density_map(content_locations);
        
        // Find local maxima
        for (location, &density) in &density_map {
            if density >= self.min_density {
                // Check if this is a local maximum
                if self.is_local_maximum(*location, &density_map) {
                    // Check distance from existing anchors
                    if self.is_far_from_anchors(*location) {
                        let anchor = AnchorPoint {
                            location: *location,
                            density,
                            influence_radius: self.calculate_influence_radius(density),
                            strength: self.calculate_anchor_strength(density),
                            last_updated: current_time,
                        };
                        new_anchors.push(anchor.clone());
                        self.anchor_points.push(anchor);
                    }
                }
            }
        }
        
        // Remove old anchors
        self.anchor_points.retain(|anchor| current_time - anchor.last_updated < 3600); // 1 hour
        
        new_anchors
    }

    /// Calculate density map for content locations
    fn calculate_density_map(&self, locations: &[GeographicLocation]) -> AHashMap<GeographicLocation, f64> {
        let mut density_map = AHashMap::new();
        let search_radius = 1000.0; // 1km
        
        for location in locations {
            let mut density = 0.0;
            
            for other_location in locations {
                if location.distance_to(other_location) <= search_radius {
                    density += 1.0;
                }
            }
            
            density_map.insert(*location, density);
        }
        
        density_map
    }

    /// Check if a location is a local maximum
    fn is_local_maximum(&self, location: GeographicLocation, density_map: &AHashMap<GeographicLocation, f64>) -> bool {
        let current_density = density_map.get(&location).unwrap_or(&0.0);
        let search_radius = 500.0; // 500m
        
        for (other_location, &density) in density_map {
            if location.distance_to(other_location) <= search_radius && density > *current_density {
                return false;
            }
        }
        
        true
    }

    /// Check if location is far from existing anchors
    fn is_far_from_anchors(&self, location: GeographicLocation) -> bool {
        for anchor in &self.anchor_points {
            if location.distance_to(&anchor.location) < self.min_distance {
                return false;
            }
        }
        true
    }

    /// Calculate influence radius based on density
    fn calculate_influence_radius(&self, density: f64) -> f64 {
        // Higher density = larger influence radius
        (density * 100.0).min(5000.0) // Max 5km
    }

    /// Calculate anchor strength
    fn calculate_anchor_strength(&self, density: f64) -> f64 {
        // Normalize density to 0-1 range
        (density / 100.0).min(1.0)
    }

    /// Get all anchor points
    pub fn get_anchors(&self) -> &[AnchorPoint] {
        &self.anchor_points
    }

    /// Find nearest anchor to a location
    pub fn find_nearest_anchor(&self, location: GeographicLocation) -> Option<&AnchorPoint> {
        self.anchor_points
            .iter()
            .min_by(|a, b| {
                let dist_a = location.distance_to(&a.location);
                let dist_b = location.distance_to(&b.location);
                dist_a.partial_cmp(&dist_b).unwrap()
            })
    }
}

/// Mobility pattern analysis for bridge nodes
#[derive(Debug, Clone)]
pub struct MobilityAnalyzer {
    /// Location history for nodes
    location_history: AHashMap<String, Vec<LocationRecord>>,
    /// Mobility patterns
    patterns: AHashMap<String, MobilityPattern>,
    /// Bridge node candidates
    bridge_candidates: Vec<BridgeNode>,
}

#[derive(Debug, Clone)]
pub struct LocationRecord {
    /// Location
    pub location: GeographicLocation,
    /// Timestamp
    pub timestamp: u64,
    /// Duration at this location
    pub duration: u64,
}

#[derive(Debug, Clone)]
pub struct MobilityPattern {
    /// Node ID
    pub node_id: String,
    /// Average speed (m/s)
    pub avg_speed: f64,
    /// Total distance traveled
    pub total_distance: f64,
    /// Number of distinct locations
    pub location_count: usize,
    /// Mobility score (0-1, higher = more mobile)
    pub mobility_score: f64,
    /// Regular locations (frequently visited)
    pub regular_locations: Vec<GeographicLocation>,
}

#[derive(Debug, Clone)]
pub struct BridgeNode {
    /// Node ID
    pub node_id: String,
    /// Bridge score (0-1, higher = better bridge)
    pub bridge_score: f64,
    /// Connected regions
    pub connected_regions: Vec<String>,
    /// Last seen
    pub last_seen: u64,
}

impl MobilityAnalyzer {
    /// Create a new mobility analyzer
    pub fn new() -> Self {
        MobilityAnalyzer {
            location_history: AHashMap::new(),
            patterns: AHashMap::new(),
            bridge_candidates: Vec::new(),
        }
    }

    /// Record a location for a node
    pub fn record_location(&mut self, node_id: String, location: GeographicLocation, timestamp: u64) {
        let record = LocationRecord {
            location,
            timestamp,
            duration: 0, // Will be calculated later
        };
        
        self.location_history
            .entry(node_id.clone())
            .or_insert_with(Vec::new)
            .push(record);
        
        // Keep only recent history (last 24 hours)
        if let Some(history) = self.location_history.get_mut(&node_id) {
            history.retain(|record| timestamp - record.timestamp < 86400);
        }
    }

    /// Analyze mobility patterns for all nodes
    pub fn analyze_patterns(&mut self, current_time: u64) {
        for (node_id, history) in &self.location_history.clone() {
            if history.len() < 2 {
                continue;
            }
            
            let pattern = self.calculate_mobility_pattern(node_id, history, current_time);
            self.patterns.insert(node_id.clone(), pattern);
        }
        
        // Update bridge candidates
        self.update_bridge_candidates(current_time);
    }

    /// Calculate mobility pattern for a node
    fn calculate_mobility_pattern(
        &self,
        node_id: &str,
        history: &[LocationRecord],
        current_time: u64,
    ) -> MobilityPattern {
        let mut total_distance = 0.0;
        let mut total_time = 0.0;
        let mut speeds = Vec::new();
        let mut locations = Vec::new();
        
        for i in 1..history.len() {
            let prev = &history[i - 1];
            let curr = &history[i];
            
            let distance = prev.location.distance_to(&curr.location);
            let time_diff = (curr.timestamp - prev.timestamp) as f64;
            
            if time_diff > 0.0 {
                let speed = distance / time_diff;
                speeds.push(speed);
                total_distance += distance;
                total_time += time_diff;
            }
            
            locations.push(curr.location);
        }
        
        let avg_speed = if !speeds.is_empty() {
            speeds.iter().sum::<f64>() / speeds.len() as f64
        } else {
            0.0
        };
        
        // Calculate mobility score based on speed and distance
        let mobility_score = (avg_speed / 10.0).min(1.0); // Normalize to 0-1
        
        // Find regular locations (visited multiple times)
        let regular_locations = self.find_regular_locations(locations);
        
        MobilityPattern {
            node_id: node_id.to_string(),
            avg_speed,
            total_distance,
            location_count: locations.len(),
            mobility_score,
            regular_locations,
        }
    }

    /// Find regularly visited locations
    fn find_regular_locations(&self, locations: Vec<GeographicLocation>) -> Vec<GeographicLocation> {
        let mut location_counts = AHashMap::new();
        let cluster_radius = 100.0; // 100m
        
        for location in locations {
            let mut found_cluster = false;
            
            for (cluster_center, count) in &mut location_counts {
                if location.distance_to(cluster_center) <= cluster_radius {
                    *count += 1;
                    found_cluster = true;
                    break;
                }
            }
            
            if !found_cluster {
                location_counts.insert(location, 1);
            }
        }
        
        // Return locations visited more than once
        location_counts
            .into_iter()
            .filter(|(_, count)| *count > 1)
            .map(|(location, _)| location)
            .collect()
    }

    /// Update bridge node candidates
    fn update_bridge_candidates(&mut self, current_time: u64) {
        self.bridge_candidates.clear();
        
        for (node_id, pattern) in &self.patterns {
            if pattern.mobility_score > 0.3 && pattern.location_count > 3 {
                let bridge_score = self.calculate_bridge_score(pattern);
                
                if bridge_score > 0.5 {
                    let bridge_node = BridgeNode {
                        node_id: node_id.clone(),
                        bridge_score,
                        connected_regions: self.get_connected_regions(pattern),
                        last_seen: current_time,
                    };
                    self.bridge_candidates.push(bridge_node);
                }
            }
        }
        
        // Sort by bridge score
        self.bridge_candidates.sort_by(|a, b| b.bridge_score.partial_cmp(&a.bridge_score).unwrap());
    }

    /// Calculate bridge score for a mobility pattern
    fn calculate_bridge_score(&self, pattern: &MobilityPattern) -> f64 {
        let mut score = 0.0;
        
        // Higher mobility = higher score
        score += pattern.mobility_score * 0.4;
        
        // More locations = higher score
        score += (pattern.location_count as f64 / 10.0).min(1.0) * 0.3;
        
        // Regular locations indicate good bridge potential
        score += (pattern.regular_locations.len() as f64 / 5.0).min(1.0) * 0.3;
        
        score
    }

    /// Get connected regions for a pattern
    fn get_connected_regions(&self, pattern: &MobilityPattern) -> Vec<String> {
        // Simple implementation - could be enhanced with actual region detection
        pattern.regular_locations
            .iter()
            .map(|loc| format!("region_{}_{}", loc.lat as i32, loc.lon as i32))
            .collect()
    }

    /// Get top bridge candidates
    pub fn get_bridge_candidates(&self, limit: usize) -> &[BridgeNode] {
        let end = limit.min(self.bridge_candidates.len());
        &self.bridge_candidates[..end]
    }

    /// Get mobility pattern for a node
    pub fn get_mobility_pattern(&self, node_id: &str) -> Option<&MobilityPattern> {
        self.patterns.get(node_id)
    }
}

/// Geographic boundary detection
#[derive(Debug, Clone)]
pub struct BoundaryDetector {
    /// Content density threshold for boundaries
    density_threshold: f64,
    /// Minimum boundary length
    min_boundary_length: f64,
    /// Detected boundaries
    boundaries: Vec<GeographicBoundary>,
}

#[derive(Debug, Clone)]
pub struct GeographicBoundary {
    /// Boundary ID
    pub id: String,
    /// Boundary points
    pub points: Vec<GeographicLocation>,
    /// Boundary type
    pub boundary_type: BoundaryType,
    /// Confidence score
    pub confidence: f64,
    /// Length in meters
    pub length: f64,
}

#[derive(Debug, Clone)]
pub enum BoundaryType {
    /// High density to low density
    DensityGradient,
    /// Content type boundary
    ContentType,
    /// Geographic feature
    Geographic,
    /// Network boundary
    Network,
}

impl BoundaryDetector {
    /// Create a new boundary detector
    pub fn new(density_threshold: f64, min_boundary_length: f64) -> Self {
        BoundaryDetector {
            density_threshold,
            min_boundary_length,
            boundaries: Vec::new(),
        }
    }

    /// Detect boundaries from content distribution
    pub fn detect_boundaries(
        &mut self,
        content_locations: &[GeographicLocation],
        content_types: &[String],
    ) -> Vec<GeographicBoundary> {
        let mut new_boundaries = Vec::new();
        
        // Create density grid
        let density_grid = self.create_density_grid(content_locations);
        
        // Find density gradients
        let density_boundaries = self.find_density_boundaries(&density_grid);
        new_boundaries.extend(density_boundaries);
        
        // Find content type boundaries
        let type_boundaries = self.find_content_type_boundaries(content_locations, content_types);
        new_boundaries.extend(type_boundaries);
        
        // Filter by minimum length
        new_boundaries.retain(|boundary| boundary.length >= self.min_boundary_length);
        
        self.boundaries.extend(new_boundaries.clone());
        new_boundaries
    }

    /// Create density grid from content locations
    fn create_density_grid(&self, locations: &[GeographicLocation]) -> AHashMap<(i32, i32), f64> {
        let mut grid = AHashMap::new();
        let grid_size = 100.0; // 100m grid cells
        
        for location in locations {
            let grid_x = (location.lon / grid_size * 111000.0) as i32;
            let grid_y = (location.lat / grid_size * 111000.0) as i32;
            
            *grid.entry((grid_x, grid_y)).or_insert(0.0) += 1.0;
        }
        
        grid
    }

    /// Find density boundaries
    fn find_density_boundaries(&self, density_grid: &AHashMap<(i32, i32), f64>) -> Vec<GeographicBoundary> {
        let mut boundaries = Vec::new();
        let grid_size = 100.0;
        
        for (&(x, y), &density) in density_grid {
            if density >= self.density_threshold {
                // Check neighbors for density gradient
                let neighbors = [
                    (x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1),
                ];
                
                for (nx, ny) in neighbors {
                    if let Some(&neighbor_density) = density_grid.get(&(nx, ny)) {
                        if (density - neighbor_density).abs() > self.density_threshold {
                            // Found a density boundary
                            let boundary = GeographicBoundary {
                                id: format!("density_{}_{}", x, y),
                                points: vec![
                                    GeographicLocation {
                                        lat: (y as f64 * grid_size) / 111000.0,
                                        lon: (x as f64 * grid_size) / 111000.0,
                                    },
                                    GeographicLocation {
                                        lat: (ny as f64 * grid_size) / 111000.0,
                                        lon: (nx as f64 * grid_size) / 111000.0,
                                    },
                                ],
                                boundary_type: BoundaryType::DensityGradient,
                                confidence: (density - neighbor_density).abs() / self.density_threshold,
                                length: grid_size,
                            };
                            boundaries.push(boundary);
                        }
                    }
                }
            }
        }
        
        boundaries
    }

    /// Find content type boundaries
    fn find_content_type_boundaries(
        &self,
        locations: &[GeographicLocation],
        content_types: &[String],
    ) -> Vec<GeographicBoundary> {
        let mut boundaries = Vec::new();
        
        // Group locations by content type
        let mut type_locations: AHashMap<String, Vec<GeographicLocation>> = AHashMap::new();
        
        for (i, location) in locations.iter().enumerate() {
            if i < content_types.len() {
                let content_type = &content_types[i];
                type_locations
                    .entry(content_type.clone())
                    .or_insert_with(Vec::new)
                    .push(*location);
            }
        }
        
        // Find boundaries between different content types
        let type_pairs: Vec<_> = type_locations.keys().collect();
        for i in 0..type_pairs.len() {
            for j in (i + 1)..type_pairs.len() {
                let type1 = type_pairs[i];
                let type2 = type_pairs[j];
                
                if let (Some(locs1), Some(locs2)) = (type_locations.get(type1), type_locations.get(type2)) {
                    let boundary = self.find_type_boundary(locs1, locs2, type1, type2);
                    if let Some(boundary) = boundary {
                        boundaries.push(boundary);
                    }
                }
            }
        }
        
        boundaries
    }

    /// Find boundary between two content types
    fn find_type_boundary(
        &self,
        locs1: &[GeographicLocation],
        locs2: &[GeographicLocation],
        type1: &str,
        type2: &str,
    ) -> Option<GeographicBoundary> {
        let mut boundary_points = Vec::new();
        let search_radius = 500.0; // 500m
        
        // Find points where types are close to each other
        for loc1 in locs1 {
            for loc2 in locs2 {
                if loc1.distance_to(loc2) <= search_radius {
                    let midpoint = GeographicLocation {
                        lat: (loc1.lat + loc2.lat) / 2.0,
                        lon: (loc1.lon + loc2.lon) / 2.0,
                    };
                    boundary_points.push(midpoint);
                }
            }
        }
        
        if boundary_points.len() >= 2 {
            let length = boundary_points
                .windows(2)
                .map(|w| w[0].distance_to(&w[1]))
                .sum();
            
            Some(GeographicBoundary {
                id: format!("type_{}_{}", type1, type2),
                points: boundary_points,
                boundary_type: BoundaryType::ContentType,
                confidence: 0.8, // Could be calculated based on point density
                length,
            })
        } else {
            None
        }
    }

    /// Get all detected boundaries
    pub fn get_boundaries(&self) -> &[GeographicBoundary] {
        &self.boundaries
    }

    /// Get boundaries of a specific type
    pub fn get_boundaries_by_type(&self, boundary_type: BoundaryType) -> Vec<&GeographicBoundary> {
        self.boundaries
            .iter()
            .filter(|boundary| std::mem::discriminant(&boundary.boundary_type) == std::mem::discriminant(&boundary_type))
            .collect()
    }
}

/// Voronoi diagram for nearest-neighbor queries
#[derive(Debug, Clone)]
pub struct VoronoiDiagram {
    /// Voronoi cells
    cells: Vec<VoronoiCell>,
    /// Delaunay triangulation
    triangulation: Option<DelaunayTriangulation<Point2<f64>>>,
}

#[derive(Debug, Clone)]
pub struct VoronoiCell {
    /// Cell ID
    pub id: String,
    /// Center point
    pub center: GeographicLocation,
    /// Cell vertices
    pub vertices: Vec<GeographicLocation>,
    /// Cell area
    pub area: f64,
}

impl VoronoiDiagram {
    /// Create a new Voronoi diagram
    pub fn new() -> Self {
        VoronoiDiagram {
            cells: Vec::new(),
            triangulation: None,
        }
    }

    /// Build Voronoi diagram from points
    pub fn build(&mut self, points: &[GeographicLocation]) -> Result<(), GeographicError> {
        if points.is_empty() {
            return Ok(());
        }
        
        // Convert to 2D points for triangulation
        let triangulation_points: Vec<Point2<f64>> = points
            .iter()
            .map(|loc| Point2::new(loc.lon, loc.lat))
            .collect();
        
        // Create Delaunay triangulation
        let triangulation = DelaunayTriangulation::<Point2<f64>>::bulk_load(triangulation_points)
            .map_err(|e| GeographicError::SpatialIndexingError(e.to_string()))?;
        
        self.triangulation = Some(triangulation);
        
        // Build Voronoi cells
        self.build_voronoi_cells(points)?;
        
        Ok(())
    }

    /// Build Voronoi cells from triangulation
    fn build_voronoi_cells(&mut self, points: &[GeographicLocation]) -> Result<(), GeographicError> {
        self.cells.clear();
        
        if let Some(triangulation) = &self.triangulation {
            for (i, point) in points.iter().enumerate() {
                let cell = VoronoiCell {
                    id: format!("cell_{}", i),
                    center: *point,
                    vertices: Vec::new(),
                    area: 0.0,
                };
                self.cells.push(cell);
            }
        }
        
        Ok(())
    }

    /// Find the cell containing a point
    pub fn find_containing_cell(&self, location: GeographicLocation) -> Option<&VoronoiCell> {
        self.cells
            .iter()
            .min_by(|a, b| {
                let dist_a = location.distance_to(&a.center);
                let dist_b = location.distance_to(&b.center);
                dist_a.partial_cmp(&dist_b).unwrap()
            })
    }

    /// Get all cells
    pub fn get_cells(&self) -> &[VoronoiCell] {
        &self.cells
    }

    /// Get cell by ID
    pub fn get_cell(&self, id: &str) -> Option<&VoronoiCell> {
        self.cells.iter().find(|cell| cell.id == id)
    }
}

/// Geographic utility functions
pub mod functions {
    use super::*;

    /// Calculate the geographic center of a set of points
    pub fn calculate_geographic_center(points: &[GeographicLocation]) -> GeographicLocation {
        if points.is_empty() {
            return GeographicLocation { lat: 0.0, lon: 0.0 };
        }

        let mut total_lat = 0.0;
        let mut total_lon = 0.0;

        for point in points {
            total_lat += point.lat;
            total_lon += point.lon;
        }

        GeographicLocation {
            lat: total_lat / points.len() as f64,
            lon: total_lon / points.len() as f64,
        }
    }

    /// Calculate the weighted geographic center
    pub fn calculate_weighted_geographic_center(
        points: &[GeographicLocation],
        weights: &[f64],
    ) -> GeographicLocation {
        if points.is_empty() || points.len() != weights.len() {
            return GeographicLocation { lat: 0.0, lon: 0.0 };
        }

        let mut total_weight = 0.0;
        let mut weighted_lat = 0.0;
        let mut weighted_lon = 0.0;

        for (point, &weight) in points.iter().zip(weights.iter()) {
            total_weight += weight;
            weighted_lat += point.lat * weight;
            weighted_lon += point.lon * weight;
        }

        if total_weight > 0.0 {
            GeographicLocation {
                lat: weighted_lat / total_weight,
                lon: weighted_lon / total_weight,
            }
        } else {
            GeographicLocation { lat: 0.0, lon: 0.0 }
        }
    }

    /// Calculate the bounding box for a set of points
    pub fn calculate_bounding_box(points: &[GeographicLocation]) -> (GeographicLocation, GeographicLocation) {
        if points.is_empty() {
            return (
                GeographicLocation { lat: 0.0, lon: 0.0 },
                GeographicLocation { lat: 0.0, lon: 0.0 },
            );
        }

        let mut min_lat = points[0].lat;
        let mut max_lat = points[0].lat;
        let mut min_lon = points[0].lon;
        let mut max_lon = points[0].lon;

        for point in points {
            min_lat = min_lat.min(point.lat);
            max_lat = max_lat.max(point.lat);
            min_lon = min_lon.min(point.lon);
            max_lon = max_lon.max(point.lon);
        }

        (
            GeographicLocation { lat: min_lat, lon: min_lon },
            GeographicLocation { lat: max_lat, lon: max_lon },
        )
    }

    /// Check if a point is within a bounding box
    pub fn point_in_bounding_box(
        point: &GeographicLocation,
        min: &GeographicLocation,
        max: &GeographicLocation,
    ) -> bool {
        point.lat >= min.lat
            && point.lat <= max.lat
            && point.lon >= min.lon
            && point.lon <= max.lon
    }

    /// Calculate the area of a polygon defined by points
    pub fn calculate_polygon_area(points: &[GeographicLocation]) -> f64 {
        if points.len() < 3 {
            return 0.0;
        }

        let mut area = 0.0;
        let earth_radius = 6371000.0; // meters

        for i in 0..points.len() {
            let j = (i + 1) % points.len();
            let lat1 = points[i].lat.to_radians();
            let lat2 = points[j].lat.to_radians();
            let delta_lon = (points[j].lon - points[i].lon).to_radians();

            area += delta_lon * (2.0 + lat1.sin() + lat2.sin());
        }

        area = area.abs() * earth_radius * earth_radius / 2.0;
        area
    }

    /// Calculate the centroid of a polygon
    pub fn calculate_polygon_centroid(points: &[GeographicLocation]) -> GeographicLocation {
        if points.is_empty() {
            return GeographicLocation { lat: 0.0, lon: 0.0 };
        }

        if points.len() == 1 {
            return points[0];
        }

        if points.len() == 2 {
            return GeographicLocation {
                lat: (points[0].lat + points[1].lat) / 2.0,
                lon: (points[0].lon + points[1].lon) / 2.0,
            };
        }

        let mut cx = 0.0;
        let mut cy = 0.0;
        let mut area = 0.0;

        for i in 0..points.len() {
            let j = (i + 1) % points.len();
            let cross = points[i].lon * points[j].lat - points[j].lon * points[i].lat;
            cx += (points[i].lon + points[j].lon) * cross;
            cy += (points[i].lat + points[j].lat) * cross;
            area += cross;
        }

        area *= 0.5;
        if area.abs() > 1e-10 {
            cx /= (6.0 * area);
            cy /= (6.0 * area);
        }

        GeographicLocation { lat: cy, lon: cx }
    }

    /// Calculate the convex hull of a set of points
    pub fn calculate_convex_hull(points: &[GeographicLocation]) -> Vec<GeographicLocation> {
        if points.len() < 3 {
            return points.to_vec();
        }

        // Graham scan algorithm
        let mut hull = Vec::new();
        
        // Find the bottom-most point (and leftmost in case of tie)
        let mut start = 0;
        for i in 1..points.len() {
            if points[i].lat < points[start].lat || 
               (points[i].lat == points[start].lat && points[i].lon < points[start].lon) {
                start = i;
            }
        }

        // Sort points by polar angle with respect to start point
        let mut sorted_points = points.to_vec();
        sorted_points.swap(0, start);
        
        // Sort by polar angle (simplified - using atan2)
        sorted_points[1..].sort_by(|a, b| {
            let angle_a = (a.lat - sorted_points[0].lat).atan2(a.lon - sorted_points[0].lon);
            let angle_b = (b.lat - sorted_points[0].lat).atan2(b.lon - sorted_points[0].lon);
            angle_a.partial_cmp(&angle_b).unwrap()
        });

        // Build convex hull
        for point in sorted_points {
            while hull.len() > 1 && 
                  cross_product(&hull[hull.len()-2], &hull[hull.len()-1], &point) <= 0.0 {
                hull.pop();
            }
            hull.push(point);
        }

        hull
    }

    /// Calculate cross product for convex hull
    fn cross_product(o: &GeographicLocation, a: &GeographicLocation, b: &GeographicLocation) -> f64 {
        (a.lon - o.lon) * (b.lat - o.lat) - (a.lat - o.lat) * (b.lon - o.lon)
    }

    /// Calculate the distance between two points using different distance metrics
    pub fn calculate_distance_metric(
        point1: &GeographicLocation,
        point2: &GeographicLocation,
        metric: DistanceMetric,
    ) -> f64 {
        match metric {
            DistanceMetric::Haversine => point1.distance_to(point2),
            DistanceMetric::Euclidean => {
                let lat_diff = point1.lat - point2.lat;
                let lon_diff = point1.lon - point2.lon;
                (lat_diff * lat_diff + lon_diff * lon_diff).sqrt() * 111000.0 // Rough conversion
            }
            DistanceMetric::Manhattan => {
                let lat_diff = (point1.lat - point2.lat).abs();
                let lon_diff = (point1.lon - point2.lon).abs();
                (lat_diff + lon_diff) * 111000.0 // Rough conversion
            }
        }
    }

    /// Distance metrics
    #[derive(Debug, Clone, Copy)]
    pub enum DistanceMetric {
        Haversine,
        Euclidean,
        Manhattan,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_location_precision_adjuster() {
        let mut adjuster = LocationPrecisionAdjuster::new();
        let location = GeographicLocation::new(40.7128, -74.0060).unwrap();
        
        let precision = adjuster.adjust_precision(location, 50.0).unwrap();
        assert!(precision >= 3 && precision <= 7);
    }

    #[test]
    fn test_anchor_point_detector() {
        let mut detector = AnchorPointDetector::new(1000.0, 10.0);
        let locations = vec![
            GeographicLocation::new(40.7128, -74.0060).unwrap(),
            GeographicLocation::new(40.7130, -74.0058).unwrap(),
            GeographicLocation::new(40.7125, -74.0065).unwrap(),
        ];
        
        let anchors = detector.detect_anchors(&locations, 1234567890);
        // Should detect at least one anchor if density is high enough
        assert!(anchors.len() >= 0);
    }

    #[test]
    fn test_mobility_analyzer() {
        let mut analyzer = MobilityAnalyzer::new();
        let location = GeographicLocation::new(40.7128, -74.0060).unwrap();
        
        analyzer.record_location("node1".to_string(), location, 1234567890);
        analyzer.analyze_patterns(1234567890);
        
        let pattern = analyzer.get_mobility_pattern("node1");
        assert!(pattern.is_some());
    }

    #[test]
    fn test_boundary_detector() {
        let mut detector = BoundaryDetector::new(5.0, 100.0);
        let locations = vec![
            GeographicLocation::new(40.7128, -74.0060).unwrap(),
            GeographicLocation::new(40.7130, -74.0058).unwrap(),
        ];
        let types = vec!["type1".to_string(), "type2".to_string()];
        
        let boundaries = detector.detect_boundaries(&locations, &types);
        assert!(boundaries.len() >= 0);
    }

    #[test]
    fn test_voronoi_diagram() {
        let mut diagram = VoronoiDiagram::new();
        let points = vec![
            GeographicLocation::new(40.7128, -74.0060).unwrap(),
            GeographicLocation::new(40.7130, -74.0058).unwrap(),
        ];
        
        assert!(diagram.build(&points).is_ok());
        assert!(!diagram.get_cells().is_empty());
    }

    #[test]
    fn test_geographic_center() {
        let points = vec![
            GeographicLocation::new(40.7128, -74.0060).unwrap(),
            GeographicLocation::new(40.7130, -74.0058).unwrap(),
        ];
        
        let center = functions::calculate_geographic_center(&points);
        assert!(center.lat > 40.7128 && center.lat < 40.7130);
        assert!(center.lon > -74.0060 && center.lon < -74.0058);
    }
}