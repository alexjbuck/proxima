//! Geographic infrastructure for Proxima - a decentralized geographic social network
//! 
//! This module provides core geographic types, algorithms, and spatial data structures
//! for location-based networking, content distribution, and routing.

use std::collections::HashMap;
use std::f64::consts::PI;
use std::sync::Arc;

use geo::{Point, Coordinate};
use geohash::{encode, decode, neighbors};
use haversine::haversine;
use nalgebra::{Vector2, Vector3, Matrix3};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Geographic layer enumeration for multi-scale addressing
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GeographicLayer {
    /// Hyperlocal: ~100m radius
    Hyperlocal,
    /// Neighborhood: ~1km radius  
    Neighborhood,
    /// District: ~10km radius
    District,
    /// City: ~100km radius
    City,
    /// Region: ~1000km radius
    Region,
    /// Global: worldwide
    Global,
}

impl GeographicLayer {
    /// Get the approximate radius in meters for each layer
    pub fn radius_meters(&self) -> f64 {
        match self {
            GeographicLayer::Hyperlocal => 100.0,
            GeographicLayer::Neighborhood => 1000.0,
            GeographicLayer::District => 10000.0,
            GeographicLayer::City => 100000.0,
            GeographicLayer::Region => 1000000.0,
            GeographicLayer::Global => f64::INFINITY,
        }
    }

    /// Get the geohash precision level for each layer
    pub fn geohash_precision(&self) -> usize {
        match self {
            GeographicLayer::Hyperlocal => 7,  // ~150m
            GeographicLayer::Neighborhood => 6, // ~1.2km
            GeographicLayer::District => 5,     // ~4.9km
            GeographicLayer::City => 4,         // ~19.5km
            GeographicLayer::Region => 3,       // ~78km
            GeographicLayer::Global => 1,       // ~2500km
        }
    }

    /// Get the parent layer (larger scale)
    pub fn parent(&self) -> Option<GeographicLayer> {
        match self {
            GeographicLayer::Hyperlocal => Some(GeographicLayer::Neighborhood),
            GeographicLayer::Neighborhood => Some(GeographicLayer::District),
            GeographicLayer::District => Some(GeographicLayer::City),
            GeographicLayer::City => Some(GeographicLayer::Region),
            GeographicLayer::Region => Some(GeographicLayer::Global),
            GeographicLayer::Global => None,
        }
    }

    /// Get the child layer (smaller scale)
    pub fn child(&self) -> Option<GeographicLayer> {
        match self {
            GeographicLayer::Hyperlocal => None,
            GeographicLayer::Neighborhood => Some(GeographicLayer::Hyperlocal),
            GeographicLayer::District => Some(GeographicLayer::Neighborhood),
            GeographicLayer::City => Some(GeographicLayer::District),
            GeographicLayer::Region => Some(GeographicLayer::City),
            GeographicLayer::Global => Some(GeographicLayer::Region),
        }
    }
}

/// Geographic address with multi-layer geohash support
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GeographicAddress {
    /// Base geohash for the location
    pub geohash: String,
    /// Layer-specific addresses
    pub layers: HashMap<GeographicLayer, String>,
    /// Original coordinates
    pub coordinates: GeographicLocation,
}

impl GeographicAddress {
    /// Create a new geographic address from coordinates
    pub fn new(lat: f64, lon: f64) -> Result<Self, GeographicError> {
        if !(-90.0..=90.0).contains(&lat) || !(-180.0..=180.0).contains(&lon) {
            return Err(GeographicError::InvalidCoordinates { lat, lon });
        }

        let coordinates = GeographicLocation { lat, lon };
        let geohash = encode(Coordinate { x: lon, y: lat }, 12)
            .map_err(|e| GeographicError::GeohashError(e.to_string()))?;

        let mut layers = HashMap::new();
        for layer in [
            GeographicLayer::Hyperlocal,
            GeographicLayer::Neighborhood,
            GeographicLayer::District,
            GeographicLayer::City,
            GeographicLayer::Region,
            GeographicLayer::Global,
        ] {
            let precision = layer.geohash_precision();
            let layer_hash = geohash.chars().take(precision).collect();
            layers.insert(layer, layer_hash);
        }

        Ok(GeographicAddress {
            geohash,
            layers,
            coordinates,
        })
    }

    /// Get the geohash for a specific layer
    pub fn get_layer_hash(&self, layer: GeographicLayer) -> &str {
        self.layers.get(&layer).map(|s| s.as_str()).unwrap_or(&self.geohash)
    }

    /// Get neighboring addresses for a specific layer
    pub fn get_neighbors(&self, layer: GeographicLayer) -> Result<Vec<String>, GeographicError> {
        let layer_hash = self.get_layer_hash(layer);
        neighbors(layer_hash)
            .map_err(|e| GeographicError::GeohashError(e.to_string()))
    }

    /// Check if this address is within a certain distance of another
    pub fn within_distance(&self, other: &GeographicAddress, distance_m: f64) -> bool {
        self.coordinates.distance_to(&other.coordinates) <= distance_m
    }
}

/// Geographic location with latitude and longitude
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct GeographicLocation {
    pub lat: f64,
    pub lon: f64,
}

impl GeographicLocation {
    /// Create a new geographic location
    pub fn new(lat: f64, lon: f64) -> Result<Self, GeographicError> {
        if !(-90.0..=90.0).contains(&lat) || !(-180.0..=180.0).contains(&lon) {
            return Err(GeographicError::InvalidCoordinates { lat, lon });
        }
        Ok(GeographicLocation { lat, lon })
    }

    /// Calculate distance to another location using Haversine formula
    pub fn distance_to(&self, other: &GeographicLocation) -> f64 {
        haversine(
            [self.lat, self.lon],
            [other.lat, other.lon],
            haversine::Units::Meters,
        )
    }

    /// Calculate bearing to another location
    pub fn bearing_to(&self, other: &GeographicLocation) -> f64 {
        let lat1 = self.lat.to_radians();
        let lat2 = other.lat.to_radians();
        let delta_lon = (other.lon - self.lon).to_radians();

        let y = delta_lon.sin() * lat2.cos();
        let x = lat1.cos() * lat2.sin() - lat1.sin() * lat2.cos() * delta_lon.cos();

        let bearing = y.atan2(x).to_degrees();
        (bearing + 360.0) % 360.0
    }

    /// Move a certain distance in a given bearing
    pub fn move_by(&self, distance_m: f64, bearing_deg: f64) -> GeographicLocation {
        let earth_radius = 6371000.0; // meters
        let lat1 = self.lat.to_radians();
        let lon1 = self.lon.to_radians();
        let bearing = bearing_deg.to_radians();

        let lat2 = (lat1.sin() * (distance_m / earth_radius).cos()
            + lat1.cos() * (distance_m / earth_radius).sin() * bearing.cos())
        .asin();

        let lon2 = lon1
            + ((bearing.sin() * (distance_m / earth_radius).sin() * lat1.cos())
                .atan2(
                    (distance_m / earth_radius).cos() - lat1.sin() * lat2.sin(),
                ));

        GeographicLocation {
            lat: lat2.to_degrees(),
            lon: lon2.to_degrees(),
        }
    }

    /// Convert to a Point for use with geo crate
    pub fn to_point(&self) -> Point<f64> {
        Point::new(self.lon, self.lat)
    }

    /// Convert from a Point
    pub fn from_point(point: Point<f64>) -> Self {
        GeographicLocation {
            lat: point.y(),
            lon: point.x(),
        }
    }
}

/// Geographic sector for routing organization
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GeographicSector {
    /// Sector identifier
    pub id: String,
    /// Center location of the sector
    pub center: GeographicLocation,
    /// Radius of the sector in meters
    pub radius: f64,
    /// Layer this sector belongs to
    pub layer: GeographicLayer,
    /// Parent sector (larger scale)
    pub parent: Option<String>,
    /// Child sectors (smaller scale)
    pub children: Vec<String>,
    /// Content density in this sector
    pub content_density: f64,
    /// Node density in this sector
    pub node_density: f64,
}

impl GeographicSector {
    /// Create a new geographic sector
    pub fn new(
        id: String,
        center: GeographicLocation,
        radius: f64,
        layer: GeographicLayer,
    ) -> Self {
        GeographicSector {
            id,
            center,
            radius,
            layer,
            parent: None,
            children: Vec::new(),
            content_density: 0.0,
            node_density: 0.0,
        }
    }

    /// Check if a location is within this sector
    pub fn contains(&self, location: &GeographicLocation) -> bool {
        self.center.distance_to(location) <= self.radius
    }

    /// Check if this sector overlaps with another
    pub fn overlaps(&self, other: &GeographicSector) -> bool {
        let distance = self.center.distance_to(&other.center);
        distance <= (self.radius + other.radius)
    }

    /// Calculate the area of this sector
    pub fn area(&self) -> f64 {
        PI * self.radius * self.radius
    }

    /// Update content density
    pub fn update_content_density(&mut self, content_count: usize) {
        self.content_density = content_count as f64 / self.area();
    }

    /// Update node density
    pub fn update_node_density(&mut self, node_count: usize) {
        self.node_density = node_count as f64 / self.area();
    }
}

/// Geographic routing table entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeographicRoute {
    /// Destination sector
    pub destination: String,
    /// Next hop sector
    pub next_hop: String,
    /// Geographic distance to destination
    pub distance: f64,
    /// Routing cost (combination of distance and density)
    pub cost: f64,
    /// Last updated timestamp
    pub last_updated: u64,
}

/// Geographic routing table
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeographicRoutingTable {
    /// Routes organized by layer
    pub routes: HashMap<GeographicLayer, HashMap<String, GeographicRoute>>,
    /// Local sector information
    pub local_sector: GeographicSector,
}

impl GeographicRoutingTable {
    /// Create a new routing table
    pub fn new(local_sector: GeographicSector) -> Self {
        GeographicRoutingTable {
            routes: HashMap::new(),
            local_sector,
        }
    }

    /// Add or update a route
    pub fn update_route(&mut self, layer: GeographicLayer, route: GeographicRoute) {
        self.routes
            .entry(layer)
            .or_insert_with(HashMap::new)
            .insert(route.destination.clone(), route);
    }

    /// Get the best route to a destination
    pub fn get_route(&self, layer: GeographicLayer, destination: &str) -> Option<&GeographicRoute> {
        self.routes
            .get(&layer)?
            .get(destination)
    }

    /// Get all routes for a layer
    pub fn get_layer_routes(&self, layer: GeographicLayer) -> Option<&HashMap<String, GeographicRoute>> {
        self.routes.get(&layer)
    }
}

/// Content relevance with geographic decay
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeographicContentRelevance {
    /// Content identifier
    pub content_id: String,
    /// Origin location
    pub origin: GeographicLocation,
    /// Current location
    pub current: GeographicLocation,
    /// Base relevance score
    pub base_relevance: f64,
    /// Geographic decay factor
    pub decay_factor: f64,
    /// Final relevance score
    pub relevance: f64,
}

impl GeographicContentRelevance {
    /// Calculate content relevance with geographic decay
    pub fn calculate(
        content_id: String,
        origin: GeographicLocation,
        current: GeographicLocation,
        base_relevance: f64,
        decay_factor: f64,
    ) -> Self {
        let distance = origin.distance_to(&current);
        let relevance = base_relevance * (-decay_factor * distance / 1000.0).exp();

        GeographicContentRelevance {
            content_id,
            origin,
            current,
            base_relevance,
            decay_factor,
            relevance,
        }
    }

    /// Update relevance for a new location
    pub fn update_location(&mut self, new_location: GeographicLocation) {
        self.current = new_location;
        let distance = self.origin.distance_to(&self.current);
        self.relevance = self.base_relevance * (-self.decay_factor * distance / 1000.0).exp();
    }
}

/// Geographic content gravity calculation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeographicContentGravity {
    /// Content identifier
    pub content_id: String,
    /// Gravity center location
    pub gravity_center: GeographicLocation,
    /// Gravity strength
    pub strength: f64,
    /// Influence radius
    pub influence_radius: f64,
    /// Content density at center
    pub center_density: f64,
}

impl GeographicContentGravity {
    /// Calculate content gravity for a set of content locations
    pub fn calculate(
        content_id: String,
        content_locations: &[GeographicLocation],
        influence_radius: f64,
    ) -> Self {
        if content_locations.is_empty() {
            return GeographicContentGravity {
                content_id,
                gravity_center: GeographicLocation { lat: 0.0, lon: 0.0 },
                strength: 0.0,
                influence_radius,
                center_density: 0.0,
            };
        }

        // Calculate weighted center of gravity
        let mut total_weight = 0.0;
        let mut weighted_lat = 0.0;
        let mut weighted_lon = 0.0;

        for location in content_locations {
            let weight = 1.0; // Could be based on content importance
            total_weight += weight;
            weighted_lat += location.lat * weight;
            weighted_lon += location.lon * weight;
        }

        let gravity_center = GeographicLocation {
            lat: weighted_lat / total_weight,
            lon: weighted_lon / total_weight,
        };

        // Calculate gravity strength based on content density
        let center_density = content_locations.len() as f64;
        let strength = center_density / (influence_radius * influence_radius * PI);

        GeographicContentGravity {
            content_id,
            gravity_center,
            strength,
            influence_radius,
            center_density,
        }
    }

    /// Calculate gravitational force at a given location
    pub fn force_at(&self, location: &GeographicLocation) -> f64 {
        let distance = self.gravity_center.distance_to(location);
        if distance > self.influence_radius {
            return 0.0;
        }

        // Inverse square law with cutoff at influence radius
        let normalized_distance = distance / self.influence_radius;
        self.strength / (1.0 + normalized_distance * normalized_distance)
    }
}

/// Geographic errors
#[derive(Error, Debug)]
pub enum GeographicError {
    #[error("Invalid coordinates: lat={lat}, lon={lon}")]
    InvalidCoordinates { lat: f64, lon: f64 },
    
    #[error("Geohash error: {0}")]
    GeohashError(String),
    
    #[error("Spatial indexing error: {0}")]
    SpatialIndexingError(String),
    
    #[error("Routing error: {0}")]
    RoutingError(String),
}

/// Geographic utility functions
pub mod utils {
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_geographic_location_creation() {
        let loc = GeographicLocation::new(40.7128, -74.0060).unwrap();
        assert_eq!(loc.lat, 40.7128);
        assert_eq!(loc.lon, -74.0060);
    }

    #[test]
    fn test_geographic_location_invalid() {
        let result = GeographicLocation::new(91.0, 0.0);
        assert!(result.is_err());
    }

    #[test]
    fn test_distance_calculation() {
        let nyc = GeographicLocation::new(40.7128, -74.0060).unwrap();
        let la = GeographicLocation::new(34.0522, -118.2437).unwrap();
        let distance = nyc.distance_to(&la);
        assert!(distance > 3900000.0 && distance < 4000000.0); // ~3944km
    }

    #[test]
    fn test_geographic_address_creation() {
        let addr = GeographicAddress::new(40.7128, -74.0060).unwrap();
        assert_eq!(addr.coordinates.lat, 40.7128);
        assert_eq!(addr.coordinates.lon, -74.0060);
        assert!(!addr.layers.is_empty());
    }

    #[test]
    fn test_geographic_sector_contains() {
        let center = GeographicLocation::new(40.7128, -74.0060).unwrap();
        let sector = GeographicSector::new(
            "test".to_string(),
            center,
            1000.0,
            GeographicLayer::Neighborhood,
        );
        
        let nearby = GeographicLocation::new(40.7130, -74.0058).unwrap();
        let far = GeographicLocation::new(40.7200, -74.0000).unwrap();
        
        assert!(sector.contains(&nearby));
        assert!(!sector.contains(&far));
    }

    #[test]
    fn test_content_relevance_calculation() {
        let origin = GeographicLocation::new(40.7128, -74.0060).unwrap();
        let current = GeographicLocation::new(40.7130, -74.0058).unwrap();
        
        let relevance = GeographicContentRelevance::calculate(
            "test".to_string(),
            origin,
            current,
            1.0,
            0.1,
        );
        
        assert!(relevance.relevance > 0.0);
        assert!(relevance.relevance <= 1.0);
    }
}