//! Spatial data structures and indexing for Proxima
//! 
//! This module provides advanced spatial indexing structures including:
//! - Hierarchical Triangular Mesh (HTM) for spherical indexing
//! - Enhanced quadtree implementation
//! - R-tree optimizations
//! - Voronoi diagrams
//! - Geographic bloom filters

use std::collections::{HashMap, HashSet, VecDeque};
use std::f64::consts::PI;
use std::sync::{Arc, RwLock};

use ahash::{AHashMap, AHashSet};
use bit_vec::BitVec;
use crossbeam::queue::SegQueue;
use geo::{Point, Coordinate, BoundingRect};
use nalgebra::{Vector2, Vector3, Matrix3};
use rand::Rng;
use rayon::prelude::*;
use spade::{DelaunayTriangulation, Triangulation, Point2, HasPosition};

use crate::geo::{
    GeographicLocation, GeographicLayer, GeographicSector, GeographicError,
    GeographicContentRelevance, GeographicContentGravity,
};

/// Hierarchical Triangular Mesh (HTM) for spherical indexing
#[derive(Debug, Clone)]
pub struct HTMIndex {
    /// Root triangles
    root_triangles: Vec<HTMTriangle>,
    /// Triangle lookup by ID
    triangles: AHashMap<String, HTMTriangle>,
    /// Maximum subdivision level
    max_level: u8,
}

#[derive(Debug, Clone)]
pub struct HTMTriangle {
    /// Unique identifier
    pub id: String,
    /// Level in the hierarchy (0 = root)
    pub level: u8,
    /// Three vertices of the triangle
    pub vertices: [Vector3<f64>; 3],
    /// Parent triangle ID
    pub parent: Option<String>,
    /// Child triangle IDs
    pub children: Vec<String>,
    /// Content stored in this triangle
    pub content: Vec<String>,
    /// Bounding box for quick filtering
    pub bounds: (f64, f64, f64, f64), // min_lat, max_lat, min_lon, max_lon
}

impl HTMIndex {
    /// Create a new HTM index
    pub fn new(max_level: u8) -> Self {
        let root_triangles = Self::create_root_triangles();
        let mut triangles = AHashMap::new();
        
        for triangle in &root_triangles {
            triangles.insert(triangle.id.clone(), triangle.clone());
        }

        HTMIndex {
            root_triangles,
            triangles,
            max_level,
        }
    }

    /// Create the 8 root triangles covering the sphere
    fn create_root_triangles() -> Vec<HTMTriangle> {
        let mut triangles = Vec::new();
        
        // Define the 8 octants of the sphere
        let vertices = [
            // Octant 1: +X, +Y, +Z
            [Vector3::new(1.0, 1.0, 1.0), Vector3::new(1.0, -1.0, 1.0), Vector3::new(1.0, 1.0, -1.0)],
            // Octant 2: -X, +Y, +Z
            [Vector3::new(-1.0, 1.0, 1.0), Vector3::new(-1.0, -1.0, 1.0), Vector3::new(-1.0, 1.0, -1.0)],
            // Octant 3: +X, -Y, +Z
            [Vector3::new(1.0, -1.0, 1.0), Vector3::new(-1.0, -1.0, 1.0), Vector3::new(1.0, -1.0, -1.0)],
            // Octant 4: -X, -Y, +Z
            [Vector3::new(-1.0, -1.0, 1.0), Vector3::new(1.0, -1.0, 1.0), Vector3::new(-1.0, -1.0, -1.0)],
            // Octant 5: +X, +Y, -Z
            [Vector3::new(1.0, 1.0, -1.0), Vector3::new(1.0, -1.0, -1.0), Vector3::new(1.0, 1.0, 1.0)],
            // Octant 6: -X, +Y, -Z
            [Vector3::new(-1.0, 1.0, -1.0), Vector3::new(-1.0, -1.0, -1.0), Vector3::new(-1.0, 1.0, 1.0)],
            // Octant 7: +X, -Y, -Z
            [Vector3::new(1.0, -1.0, -1.0), Vector3::new(-1.0, -1.0, -1.0), Vector3::new(1.0, -1.0, 1.0)],
            // Octant 8: -X, -Y, -Z
            [Vector3::new(-1.0, -1.0, -1.0), Vector3::new(1.0, -1.0, -1.0), Vector3::new(-1.0, -1.0, 1.0)],
        ];

        for (i, verts) in vertices.iter().enumerate() {
            let triangle = HTMTriangle {
                id: format!("R{}", i),
                level: 0,
                vertices: *verts,
                parent: None,
                children: Vec::new(),
                content: Vec::new(),
                bounds: Self::calculate_bounds(verts),
            };
            triangles.push(triangle);
        }

        triangles
    }

    /// Calculate bounding box for triangle vertices
    fn calculate_bounds(vertices: &[Vector3<f64>; 3]) -> (f64, f64, f64, f64) {
        let mut min_lat = f64::INFINITY;
        let mut max_lat = f64::NEG_INFINITY;
        let mut min_lon = f64::INFINITY;
        let mut max_lon = f64::NEG_INFINITY;

        for vertex in vertices {
            let (lat, lon) = Self::cartesian_to_spherical(*vertex);
            min_lat = min_lat.min(lat);
            max_lat = max_lat.max(lat);
            min_lon = min_lon.min(lon);
            max_lon = max_lon.max(lon);
        }

        (min_lat, max_lat, min_lon, max_lon)
    }

    /// Convert Cartesian coordinates to spherical (lat, lon)
    fn cartesian_to_spherical(point: Vector3<f64>) -> (f64, f64) {
        let lat = point.z.asin().to_degrees();
        let lon = point.y.atan2(point.x).to_degrees();
        (lat, lon)
    }

    /// Convert spherical coordinates to Cartesian
    fn spherical_to_cartesian(lat: f64, lon: f64) -> Vector3<f64> {
        let lat_rad = lat.to_radians();
        let lon_rad = lon.to_radians();
        Vector3::new(
            lat_rad.cos() * lon_rad.cos(),
            lat_rad.cos() * lon_rad.sin(),
            lat_rad.sin(),
        )
    }

    /// Insert content at a location
    pub fn insert(&mut self, location: GeographicLocation, content_id: String) -> Result<(), GeographicError> {
        let triangle_id = self.find_containing_triangle(location)?;
        self.insert_recursive(triangle_id, location, content_id, 0)
    }

    /// Find the triangle containing a location
    fn find_containing_triangle(&self, location: GeographicLocation) -> Result<String, GeographicError> {
        let point = Self::spherical_to_cartesian(location.lat, location.lon);
        
        for root_triangle in &self.root_triangles {
            if self.point_in_triangle(&point, &root_triangle.vertices) {
                return self.find_containing_triangle_recursive(&root_triangle.id, location);
            }
        }
        
        Err(GeographicError::SpatialIndexingError("Point not found in any triangle".to_string()))
    }

    /// Recursively find containing triangle
    fn find_containing_triangle_recursive(&self, triangle_id: &str, location: GeographicLocation) -> Result<String, GeographicError> {
        let triangle = self.triangles.get(triangle_id)
            .ok_or_else(|| GeographicError::SpatialIndexingError("Triangle not found".to_string()))?;

        if triangle.children.is_empty() {
            return Ok(triangle_id.to_string());
        }

        let point = Self::spherical_to_cartesian(location.lat, location.lon);
        for child_id in &triangle.children {
            let child = self.triangles.get(child_id)
                .ok_or_else(|| GeographicError::SpatialIndexingError("Child triangle not found".to_string()))?;
            
            if self.point_in_triangle(&point, &child.vertices) {
                return self.find_containing_triangle_recursive(child_id, location);
            }
        }

        Ok(triangle_id.to_string())
    }

    /// Check if a point is inside a triangle
    fn point_in_triangle(&self, point: &Vector3<f64>, vertices: &[Vector3<f64>; 3]) -> bool {
        // Use barycentric coordinates to test if point is inside triangle
        let v0 = vertices[2] - vertices[0];
        let v1 = vertices[1] - vertices[0];
        let v2 = *point - vertices[0];

        let dot00 = v0.dot(&v0);
        let dot01 = v0.dot(&v1);
        let dot02 = v0.dot(&v2);
        let dot11 = v1.dot(&v1);
        let dot12 = v1.dot(&v2);

        let inv_denom = 1.0 / (dot00 * dot11 - dot01 * dot01);
        let u = (dot11 * dot02 - dot01 * dot12) * inv_denom;
        let v = (dot00 * dot12 - dot01 * dot02) * inv_denom;

        (u >= 0.0) && (v >= 0.0) && (u + v <= 1.0)
    }

    /// Recursively insert content, subdividing triangles as needed
    fn insert_recursive(
        &mut self,
        triangle_id: String,
        location: GeographicLocation,
        content_id: String,
        level: u8,
    ) -> Result<(), GeographicError> {
        if level >= self.max_level {
            // Insert at current level
            if let Some(triangle) = self.triangles.get_mut(&triangle_id) {
                triangle.content.push(content_id);
            }
            return Ok(());
        }

        let triangle = self.triangles.get(&triangle_id)
            .ok_or_else(|| GeographicError::SpatialIndexingError("Triangle not found".to_string()))?;

        if triangle.children.is_empty() {
            // Subdivide triangle
            self.subdivide_triangle(&triangle_id)?;
        }

        // Find the child triangle containing the location
        let point = Self::spherical_to_cartesian(location.lat, location.lon);
        for child_id in &triangle.children {
            let child = self.triangles.get(child_id)
                .ok_or_else(|| GeographicError::SpatialIndexingError("Child triangle not found".to_string()))?;
            
            if self.point_in_triangle(&point, &child.vertices) {
                return self.insert_recursive(child_id.clone(), location, content_id, level + 1);
            }
        }

        // Fallback: insert at current level
        if let Some(triangle) = self.triangles.get_mut(&triangle_id) {
            triangle.content.push(content_id);
        }

        Ok(())
    }

    /// Subdivide a triangle into 4 child triangles
    fn subdivide_triangle(&mut self, triangle_id: &str) -> Result<(), GeographicError> {
        let triangle = self.triangles.get(triangle_id)
            .ok_or_else(|| GeographicError::SpatialIndexingError("Triangle not found".to_string()))?
            .clone();

        let mut children = Vec::new();
        
        // Create 4 child triangles
        for i in 0..4 {
            let child_id = format!("{}_{}", triangle_id, i);
            let child_vertices = self.calculate_child_vertices(&triangle.vertices, i);
            
            let child = HTMTriangle {
                id: child_id.clone(),
                level: triangle.level + 1,
                vertices: child_vertices,
                parent: Some(triangle_id.to_string()),
                children: Vec::new(),
                content: Vec::new(),
                bounds: Self::calculate_bounds(&child_vertices),
            };
            
            children.push(child_id.clone());
            self.triangles.insert(child_id, child);
        }

        // Update parent triangle
        if let Some(parent) = self.triangles.get_mut(triangle_id) {
            parent.children = children;
        }

        Ok(())
    }

    /// Calculate vertices for child triangles
    fn calculate_child_vertices(&self, parent_vertices: &[Vector3<f64>; 3], child_index: usize) -> [Vector3<f64>; 3] {
        let v0 = parent_vertices[0];
        let v1 = parent_vertices[1];
        let v2 = parent_vertices[2];

        // Calculate midpoints
        let m01 = (v0 + v1).normalize();
        let m12 = (v1 + v2).normalize();
        let m20 = (v2 + v0).normalize();

        match child_index {
            0 => [v0, m01, m20],
            1 => [m01, v1, m12],
            2 => [m20, m12, v2],
            3 => [m01, m12, m20],
            _ => panic!("Invalid child index"),
        }
    }

    /// Query content within a radius of a location
    pub fn query_radius(&self, center: GeographicLocation, radius: f64) -> Vec<String> {
        let mut results = Vec::new();
        let center_point = Self::spherical_to_cartesian(center.lat, center.lon);
        
        for root_triangle in &self.root_triangles {
            self.query_radius_recursive(&root_triangle.id, &center_point, radius, &mut results);
        }
        
        results
    }

    /// Recursively query radius
    fn query_radius_recursive(
        &self,
        triangle_id: &str,
        center_point: &Vector3<f64>,
        radius: f64,
        results: &mut Vec<String>,
    ) {
        if let Some(triangle) = self.triangles.get(triangle_id) {
            // Check if triangle intersects with query radius
            if self.triangle_intersects_sphere(triangle, center_point, radius) {
                // Add content from this triangle
                results.extend(triangle.content.clone());
                
                // Recursively check children
                for child_id in &triangle.children {
                    self.query_radius_recursive(child_id, center_point, radius, results);
                }
            }
        }
    }

    /// Check if triangle intersects with a sphere
    fn triangle_intersects_sphere(
        &self,
        triangle: &HTMTriangle,
        center: &Vector3<f64>,
        radius: f64,
    ) -> bool {
        // Simple bounding box check first
        let (min_lat, max_lat, min_lon, max_lon) = triangle.bounds;
        let center_lat = center.z.asin().to_degrees();
        let center_lon = center.y.atan2(center.x).to_degrees();
        
        // Convert radius to approximate lat/lon bounds
        let lat_radius = radius / 111000.0; // Rough conversion
        let lon_radius = radius / (111000.0 * center_lat.cos().abs());
        
        let intersects = center_lat - lat_radius <= max_lat
            && center_lat + lat_radius >= min_lat
            && center_lon - lon_radius <= max_lon
            && center_lon + lon_radius >= min_lon;
            
        intersects
    }
}

/// Enhanced quadtree for 2D spatial indexing
#[derive(Debug, Clone)]
pub struct QuadTree {
    /// Root node
    root: QuadNode,
    /// Maximum depth
    max_depth: u8,
    /// Maximum items per node before splitting
    max_items: usize,
}

#[derive(Debug, Clone)]
pub struct QuadNode {
    /// Bounding box
    bounds: (f64, f64, f64, f64), // min_lat, max_lat, min_lon, max_lon
    /// Items in this node
    items: Vec<QuadItem>,
    /// Child nodes (nw, ne, sw, se)
    children: Option<[Box<QuadNode>; 4]>,
    /// Depth level
    depth: u8,
}

#[derive(Debug, Clone)]
pub struct QuadItem {
    /// Item identifier
    pub id: String,
    /// Location
    pub location: GeographicLocation,
    /// Additional data
    pub data: String,
}

impl QuadTree {
    /// Create a new quadtree
    pub fn new(
        bounds: (f64, f64, f64, f64),
        max_depth: u8,
        max_items: usize,
    ) -> Self {
        QuadTree {
            root: QuadNode {
                bounds,
                items: Vec::new(),
                children: None,
                depth: 0,
            },
            max_depth,
            max_items,
        }
    }

    /// Insert an item into the quadtree
    pub fn insert(&mut self, item: QuadItem) -> Result<(), GeographicError> {
        if !self.point_in_bounds(&item.location, &self.root.bounds) {
            return Err(GeographicError::SpatialIndexingError(
                "Item outside quadtree bounds".to_string()
            ));
        }

        self.insert_recursive(&mut self.root, item)
    }

    /// Recursively insert item
    fn insert_recursive(&mut self, node: &mut QuadNode, item: QuadItem) -> Result<(), GeographicError> {
        if node.children.is_none() {
            // Leaf node
            node.items.push(item);
            
            // Check if we need to split
            if node.items.len() > self.max_items && node.depth < self.max_depth {
                self.split_node(node)?;
            }
        } else {
            // Internal node - find appropriate child
            let children = node.children.as_mut().unwrap();
            let child_index = self.get_child_index(&item.location, &node.bounds);
            self.insert_recursive(&mut children[child_index], item)?;
        }

        Ok(())
    }

    /// Split a node into 4 children
    fn split_node(&mut self, node: &mut QuadNode) -> Result<(), GeographicError> {
        let (min_lat, max_lat, min_lon, max_lon) = node.bounds;
        let mid_lat = (min_lat + max_lat) / 2.0;
        let mid_lon = (min_lon + max_lon) / 2.0;

        let children = [
            Box::new(QuadNode {
                bounds: (min_lat, mid_lat, min_lon, mid_lon), // SW
                items: Vec::new(),
                children: None,
                depth: node.depth + 1,
            }),
            Box::new(QuadNode {
                bounds: (min_lat, mid_lat, mid_lon, max_lon), // SE
                items: Vec::new(),
                children: None,
                depth: node.depth + 1,
            }),
            Box::new(QuadNode {
                bounds: (mid_lat, max_lat, min_lon, mid_lon), // NW
                items: Vec::new(),
                children: None,
                depth: node.depth + 1,
            }),
            Box::new(QuadNode {
                bounds: (mid_lat, max_lat, mid_lon, max_lon), // NE
                items: Vec::new(),
                children: None,
                depth: node.depth + 1,
            }),
        ];

        // Redistribute items to children
        let items = std::mem::take(&mut node.items);
        for item in items {
            let child_index = self.get_child_index(&item.location, &node.bounds);
            children[child_index].items.push(item);
        }

        node.children = Some(children);
        Ok(())
    }

    /// Get child index for a location
    fn get_child_index(&self, location: &GeographicLocation, bounds: &(f64, f64, f64, f64)) -> usize {
        let (min_lat, max_lat, min_lon, max_lon) = bounds;
        let mid_lat = (min_lat + max_lat) / 2.0;
        let mid_lon = (min_lon + max_lon) / 2.0;

        let lat_high = location.lat >= mid_lat;
        let lon_high = location.lon >= mid_lon;

        match (lat_high, lon_high) {
            (false, false) => 0, // SW
            (false, true) => 1,  // SE
            (true, false) => 2,  // NW
            (true, true) => 3,   // NE
        }
    }

    /// Check if point is in bounds
    fn point_in_bounds(&self, location: &GeographicLocation, bounds: &(f64, f64, f64, f64)) -> bool {
        let (min_lat, max_lat, min_lon, max_lon) = bounds;
        location.lat >= min_lat
            && location.lat <= max_lat
            && location.lon >= min_lon
            && location.lon <= max_lon
    }

    /// Query items within a radius
    pub fn query_radius(&self, center: GeographicLocation, radius: f64) -> Vec<QuadItem> {
        let mut results = Vec::new();
        self.query_radius_recursive(&self.root, center, radius, &mut results);
        results
    }

    /// Recursively query radius
    fn query_radius_recursive(
        &self,
        node: &QuadNode,
        center: GeographicLocation,
        radius: f64,
        results: &mut Vec<QuadItem>,
    ) {
        // Check if node bounds intersect with query circle
        if self.bounds_intersect_circle(&node.bounds, center, radius) {
            // Add items from this node
            for item in &node.items {
                if center.distance_to(&item.location) <= radius {
                    results.push(item.clone());
                }
            }

            // Recursively check children
            if let Some(children) = &node.children {
                for child in children {
                    self.query_radius_recursive(child, center, radius, results);
                }
            }
        }
    }

    /// Check if bounds intersect with circle
    fn bounds_intersect_circle(
        &self,
        bounds: &(f64, f64, f64, f64),
        center: GeographicLocation,
        radius: f64,
    ) -> bool {
        let (min_lat, max_lat, min_lon, max_lon) = bounds;
        
        // Convert radius to approximate lat/lon bounds
        let lat_radius = radius / 111000.0;
        let lon_radius = radius / (111000.0 * center.lat.cos().abs());
        
        center.lat - lat_radius <= max_lat
            && center.lat + lat_radius >= min_lat
            && center.lon - lon_radius <= max_lon
            && center.lon + lon_radius >= min_lon
    }
}

/// R-tree for efficient spatial queries
#[derive(Debug, Clone)]
pub struct RTree {
    /// Root node
    root: Option<RTreeNode>,
    /// Maximum entries per node
    max_entries: usize,
    /// Minimum entries per node
    min_entries: usize,
}

#[derive(Debug, Clone)]
pub struct RTreeNode {
    /// Bounding rectangle
    bounds: (f64, f64, f64, f64), // min_lat, max_lat, min_lon, max_lon
    /// Entries (either data items or child nodes)
    entries: Vec<RTEntry>,
    /// Whether this is a leaf node
    is_leaf: bool,
}

#[derive(Debug, Clone)]
pub enum RTEntry {
    /// Data entry
    Data {
        id: String,
        location: GeographicLocation,
        data: String,
    },
    /// Child node entry
    Node(Box<RTreeNode>),
}

impl RTree {
    /// Create a new R-tree
    pub fn new(max_entries: usize) -> Self {
        RTree {
            root: None,
            max_entries,
            min_entries: max_entries / 2,
        }
    }

    /// Insert an entry into the R-tree
    pub fn insert(&mut self, id: String, location: GeographicLocation, data: String) {
        let entry = RTEntry::Data { id, location, data };
        
        if let Some(root) = &mut self.root {
            self.insert_entry(root, entry);
        } else {
            // Create root node
            self.root = Some(RTreeNode {
                bounds: (location.lat, location.lat, location.lon, location.lon),
                entries: vec![entry],
                is_leaf: true,
            });
        }
    }

    /// Insert entry into a node
    fn insert_entry(&mut self, node: &mut RTreeNode, entry: RTEntry) {
        if node.is_leaf {
            node.entries.push(entry);
            
            if node.entries.len() > self.max_entries {
                self.split_node(node);
            }
        } else {
            // Find best child node
            let best_child = self.choose_subtree(&node.entries, &entry);
            if let RTEntry::Node(child) = &mut node.entries[best_child] {
                self.insert_entry(child, entry);
            }
        }
    }

    /// Choose best subtree for insertion
    fn choose_subtree(&self, entries: &[RTEntry], entry: &RTEntry) -> usize {
        let mut best_index = 0;
        let mut min_enlargement = f64::INFINITY;
        let mut min_area = f64::INFINITY;

        for (i, existing_entry) in entries.iter().enumerate() {
            let bounds = self.get_entry_bounds(existing_entry);
            let area = self.calculate_area(&bounds);
            let enlarged_bounds = self.calculate_union(&bounds, &self.get_entry_bounds(entry));
            let enlarged_area = self.calculate_area(&enlarged_bounds);
            let enlargement = enlarged_area - area;

            if enlargement < min_enlargement || 
               (enlargement == min_enlargement && area < min_area) {
                min_enlargement = enlargement;
                min_area = area;
                best_index = i;
            }
        }

        best_index
    }

    /// Get bounds for an entry
    fn get_entry_bounds(&self, entry: &RTEntry) -> (f64, f64, f64, f64) {
        match entry {
            RTEntry::Data { location, .. } => {
                (location.lat, location.lat, location.lon, location.lon)
            }
            RTEntry::Node(node) => node.bounds,
        }
    }

    /// Calculate area of bounds
    fn calculate_area(&self, bounds: &(f64, f64, f64, f64)) -> f64 {
        let (min_lat, max_lat, min_lon, max_lon) = bounds;
        (max_lat - min_lat) * (max_lon - min_lon)
    }

    /// Calculate union of two bounds
    fn calculate_union(
        &self,
        bounds1: &(f64, f64, f64, f64),
        bounds2: &(f64, f64, f64, f64),
    ) -> (f64, f64, f64, f64) {
        let (min_lat1, max_lat1, min_lon1, max_lon1) = bounds1;
        let (min_lat2, max_lat2, min_lon2, max_lon2) = bounds2;
        
        (
            min_lat1.min(*min_lat2),
            max_lat1.max(*max_lat2),
            min_lon1.min(*min_lon2),
            max_lon1.max(*max_lon2),
        )
    }

    /// Split a node when it exceeds max_entries
    fn split_node(&mut self, node: &mut RTreeNode) {
        // Simple linear split for now
        let mid = node.entries.len() / 2;
        let entries2 = node.entries.split_off(mid);
        
        let bounds1 = self.calculate_bounds(&node.entries);
        let bounds2 = self.calculate_bounds(&entries2);
        
        let child1 = RTreeNode {
            bounds: bounds1,
            entries: node.entries.clone(),
            is_leaf: node.is_leaf,
        };
        
        let child2 = RTreeNode {
            bounds: bounds2,
            entries: entries2,
            is_leaf: node.is_leaf,
        };
        
        node.entries = vec![
            RTEntry::Node(Box::new(child1)),
            RTEntry::Node(Box::new(child2)),
        ];
        node.is_leaf = false;
    }

    /// Calculate bounds for a set of entries
    fn calculate_bounds(&self, entries: &[RTEntry]) -> (f64, f64, f64, f64) {
        if entries.is_empty() {
            return (0.0, 0.0, 0.0, 0.0);
        }

        let mut min_lat = f64::INFINITY;
        let mut max_lat = f64::NEG_INFINITY;
        let mut min_lon = f64::INFINITY;
        let mut max_lon = f64::NEG_INFINITY;

        for entry in entries {
            let bounds = self.get_entry_bounds(entry);
            min_lat = min_lat.min(bounds.0);
            max_lat = max_lat.max(bounds.1);
            min_lon = min_lon.min(bounds.2);
            max_lon = max_lon.max(bounds.3);
        }

        (min_lat, max_lat, min_lon, max_lon)
    }

    /// Query entries within a radius
    pub fn query_radius(&self, center: GeographicLocation, radius: f64) -> Vec<RTEntry> {
        let mut results = Vec::new();
        
        if let Some(root) = &self.root {
            self.query_radius_recursive(root, center, radius, &mut results);
        }
        
        results
    }

    /// Recursively query radius
    fn query_radius_recursive(
        &self,
        node: &RTreeNode,
        center: GeographicLocation,
        radius: f64,
        results: &mut Vec<RTEntry>,
    ) {
        // Check if node bounds intersect with query circle
        if self.bounds_intersect_circle(&node.bounds, center, radius) {
            for entry in &node.entries {
                match entry {
                    RTEntry::Data { location, .. } => {
                        if center.distance_to(location) <= radius {
                            results.push(entry.clone());
                        }
                    }
                    RTEntry::Node(child) => {
                        self.query_radius_recursive(child, center, radius, results);
                    }
                }
            }
        }
    }

    /// Check if bounds intersect with circle
    fn bounds_intersect_circle(
        &self,
        bounds: &(f64, f64, f64, f64),
        center: GeographicLocation,
        radius: f64,
    ) -> bool {
        let (min_lat, max_lat, min_lon, max_lon) = bounds;
        
        // Convert radius to approximate lat/lon bounds
        let lat_radius = radius / 111000.0;
        let lon_radius = radius / (111000.0 * center.lat.cos().abs());
        
        center.lat - lat_radius <= max_lat
            && center.lat + lat_radius >= min_lat
            && center.lon - lon_radius <= max_lon
            && center.lon + lon_radius >= min_lon
    }
}

/// Geographic bloom filter for efficient content presence queries
#[derive(Debug, Clone)]
pub struct GeographicBloomFilter {
    /// Bit vector
    bits: BitVec,
    /// Number of hash functions
    hash_functions: usize,
    /// Expected number of items
    expected_items: usize,
    /// False positive rate
    false_positive_rate: f64,
}

impl GeographicBloomFilter {
    /// Create a new geographic bloom filter
    pub fn new(expected_items: usize, false_positive_rate: f64) -> Self {
        let bit_count = Self::calculate_bit_count(expected_items, false_positive_rate);
        let hash_functions = Self::calculate_hash_functions(bit_count, expected_items);
        
        GeographicBloomFilter {
            bits: BitVec::from_elem(bit_count, false),
            hash_functions,
            expected_items,
            false_positive_rate,
        }
    }

    /// Calculate optimal bit count
    fn calculate_bit_count(expected_items: usize, false_positive_rate: f64) -> usize {
        let ln2 = 2.0_f64.ln();
        ((-(expected_items as f64) * false_positive_rate.ln()) / (ln2 * ln2)).ceil() as usize
    }

    /// Calculate optimal number of hash functions
    fn calculate_hash_functions(bit_count: usize, expected_items: usize) -> usize {
        let ln2 = 2.0_f64.ln();
        ((bit_count as f64 / expected_items as f64) * ln2).round() as usize
    }

    /// Add a location to the bloom filter
    pub fn add(&mut self, location: GeographicLocation) {
        for i in 0..self.hash_functions {
            let hash = self.hash_location(&location, i);
            let index = hash % self.bits.len();
            self.bits.set(index, true);
        }
    }

    /// Check if a location might be in the bloom filter
    pub fn might_contain(&self, location: GeographicLocation) -> bool {
        for i in 0..self.hash_functions {
            let hash = self.hash_location(&location, i);
            let index = hash % self.bits.len();
            if !self.bits[index] {
                return false;
            }
        }
        true
    }

    /// Hash a location with a specific seed
    fn hash_location(&self, location: &GeographicLocation, seed: usize) -> usize {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let mut hasher = DefaultHasher::new();
        (location.lat * 1000000.0) as i64.hash(&mut hasher);
        (location.lon * 1000000.0) as i64.hash(&mut hasher);
        seed.hash(&mut hasher);
        hasher.finish() as usize
    }

    /// Get the current false positive rate
    pub fn current_false_positive_rate(&self) -> f64 {
        let set_bits = self.bits.iter().filter(|&b| b).count();
        let ratio = set_bits as f64 / self.bits.len() as f64;
        ratio.powi(self.hash_functions as i32)
    }
}

/// Spatial content index for efficient geographic queries
#[derive(Debug)]
pub struct SpatialContentIndex {
    /// HTM index for spherical queries
    htm_index: HTMIndex,
    /// Quadtree for 2D queries
    quadtree: QuadTree,
    /// R-tree for range queries
    rtree: RTree,
    /// Bloom filter for presence queries
    bloom_filter: GeographicBloomFilter,
    /// Content metadata
    content_metadata: AHashMap<String, ContentMetadata>,
}

#[derive(Debug, Clone)]
pub struct ContentMetadata {
    /// Content identifier
    pub id: String,
    /// Origin location
    pub origin: GeographicLocation,
    /// Content type
    pub content_type: String,
    /// Timestamp
    pub timestamp: u64,
    /// Relevance score
    pub relevance: f64,
    /// Geographic gravity
    pub gravity: Option<GeographicContentGravity>,
}

impl SpatialContentIndex {
    /// Create a new spatial content index
    pub fn new() -> Self {
        SpatialContentIndex {
            htm_index: HTMIndex::new(10),
            quadtree: QuadTree::new(
                (-90.0, 90.0, -180.0, 180.0), // Global bounds
                15, // Max depth
                10, // Max items per node
            ),
            rtree: RTree::new(10),
            bloom_filter: GeographicBloomFilter::new(1000000, 0.01), // 1M items, 1% FPR
            content_metadata: AHashMap::new(),
        }
    }

    /// Insert content into the index
    pub fn insert(&mut self, metadata: ContentMetadata) -> Result<(), GeographicError> {
        let content_id = metadata.id.clone();
        
        // Insert into HTM index
        self.htm_index.insert(metadata.origin, content_id.clone())?;
        
        // Insert into quadtree
        let quad_item = QuadItem {
            id: content_id.clone(),
            location: metadata.origin,
            data: metadata.content_type.clone(),
        };
        self.quadtree.insert(quad_item)?;
        
        // Insert into R-tree
        self.rtree.insert(
            content_id.clone(),
            metadata.origin,
            metadata.content_type.clone(),
        );
        
        // Add to bloom filter
        self.bloom_filter.add(metadata.origin);
        
        // Store metadata
        self.content_metadata.insert(content_id, metadata);
        
        Ok(())
    }

    /// Query content within a radius
    pub fn query_radius(&self, center: GeographicLocation, radius: f64) -> Vec<ContentMetadata> {
        let mut results = Vec::new();
        
        // Use bloom filter for quick elimination
        if !self.bloom_filter.might_contain(center) {
            return results;
        }
        
        // Query from HTM index
        let htm_results = self.htm_index.query_radius(center, radius);
        
        // Query from quadtree
        let quad_results = self.quadtree.query_radius(center, radius);
        
        // Query from R-tree
        let rtree_results = self.rtree.query_radius(center, radius);
        
        // Combine and deduplicate results
        let mut seen = AHashSet::new();
        
        for content_id in htm_results {
            if seen.insert(content_id.clone()) {
                if let Some(metadata) = self.content_metadata.get(&content_id) {
                    results.push(metadata.clone());
                }
            }
        }
        
        for item in quad_results {
            if seen.insert(item.id.clone()) {
                if let Some(metadata) = self.content_metadata.get(&item.id) {
                    results.push(metadata.clone());
                }
            }
        }
        
        for entry in rtree_results {
            if let RTEntry::Data { id, .. } = entry {
                if seen.insert(id.clone()) {
                    if let Some(metadata) = self.content_metadata.get(&id) {
                        results.push(metadata.clone());
                    }
                }
            }
        }
        
        results
    }

    /// Get content metadata by ID
    pub fn get_metadata(&self, content_id: &str) -> Option<&ContentMetadata> {
        self.content_metadata.get(content_id)
    }

    /// Update content relevance
    pub fn update_relevance(&mut self, content_id: &str, new_relevance: f64) {
        if let Some(metadata) = self.content_metadata.get_mut(content_id) {
            metadata.relevance = new_relevance;
        }
    }

    /// Calculate content gravity for a content type
    pub fn calculate_content_gravity(
        &self,
        content_type: &str,
        influence_radius: f64,
    ) -> GeographicContentGravity {
        let content_locations: Vec<GeographicLocation> = self.content_metadata
            .values()
            .filter(|metadata| metadata.content_type == content_type)
            .map(|metadata| metadata.origin)
            .collect();
        
        GeographicContentGravity::calculate(
            content_type.to_string(),
            &content_locations,
            influence_radius,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quadtree_insertion() {
        let mut quadtree = QuadTree::new(
            (-90.0, 90.0, -180.0, 180.0),
            5,
            2,
        );
        
        let item = QuadItem {
            id: "test".to_string(),
            location: GeographicLocation::new(40.7128, -74.0060).unwrap(),
            data: "test_data".to_string(),
        };
        
        assert!(quadtree.insert(item).is_ok());
    }

    #[test]
    fn test_quadtree_query() {
        let mut quadtree = QuadTree::new(
            (-90.0, 90.0, -180.0, 180.0),
            5,
            2,
        );
        
        let item = QuadItem {
            id: "test".to_string(),
            location: GeographicLocation::new(40.7128, -74.0060).unwrap(),
            data: "test_data".to_string(),
        };
        
        quadtree.insert(item).unwrap();
        
        let results = quadtree.query_radius(
            GeographicLocation::new(40.7130, -74.0058).unwrap(),
            1000.0,
        );
        
        assert!(!results.is_empty());
    }

    #[test]
    fn test_bloom_filter() {
        let mut bloom = GeographicBloomFilter::new(1000, 0.01);
        
        let location = GeographicLocation::new(40.7128, -74.0060).unwrap();
        bloom.add(location);
        
        assert!(bloom.might_contain(location));
    }

    #[test]
    fn test_spatial_content_index() {
        let mut index = SpatialContentIndex::new();
        
        let metadata = ContentMetadata {
            id: "test_content".to_string(),
            origin: GeographicLocation::new(40.7128, -74.0060).unwrap(),
            content_type: "post".to_string(),
            timestamp: 1234567890,
            relevance: 1.0,
            gravity: None,
        };
        
        assert!(index.insert(metadata).is_ok());
        
        let results = index.query_radius(
            GeographicLocation::new(40.7130, -74.0058).unwrap(),
            1000.0,
        );
        
        assert!(!results.is_empty());
    }
}