//! Geographic types and addressing system for Proxima

use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use geohash::{encode, decode};
use thiserror::Error;

/// Geographic layers representing different scales of interaction
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GeographicLayer {
    /// Hyperlocal: ~100m radius (same building/block)
    Hyperlocal,
    /// Neighborhood: ~1km radius (walkable distance)
    Neighborhood,
    /// District: ~5km radius (bikeable distance)
    District,
    /// City: ~25km radius (same metro area)
    City,
    /// Region: ~100km radius (cultural region)
    Region,
}

impl GeographicLayer {
    /// Get the typical radius for this geographic layer in meters
    pub fn radius_meters(&self) -> f64 {
        match self {
            GeographicLayer::Hyperlocal => 100.0,
            GeographicLayer::Neighborhood => 1000.0,
            GeographicLayer::District => 5000.0,
            GeographicLayer::City => 25000.0,
            GeographicLayer::Region => 100000.0,
        }
    }
    
    /// Get the geohash precision needed for this layer
    pub fn geohash_precision(&self) -> usize {
        match self {
            GeographicLayer::Hyperlocal => 7,  // ~150m
            GeographicLayer::Neighborhood => 6, // ~600m
            GeographicLayer::District => 5,     // ~2.4km
            GeographicLayer::City => 4,         // ~20km
            GeographicLayer::Region => 3,       // ~156km
        }
    }
    
    /// Get all layers from most local to most global
    pub fn all_layers() -> Vec<GeographicLayer> {
        vec![
            GeographicLayer::Hyperlocal,
            GeographicLayer::Neighborhood,
            GeographicLayer::District,
            GeographicLayer::City,
            GeographicLayer::Region,
        ]
    }
}

/// A geographic address for a node in the Proxima network
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GeographicAddress {
    /// Geohash representation of the location
    pub geohash: String,
    /// The geographic layer this address represents
    pub layer: GeographicLayer,
    /// Confidence in the location accuracy (0.0 to 1.0)
    pub confidence: f64,
    /// Timestamp when this location was last verified
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl GeographicAddress {
    /// Create a new geographic address
    pub fn new(
        latitude: f64,
        longitude: f64,
        layer: GeographicLayer,
        confidence: f64,
    ) -> Result<Self, GeographicError> {
        if !(-90.0..=90.0).contains(&latitude) || !(-180.0..=180.0).contains(&longitude) {
            return Err(GeographicError::InvalidCoordinates { latitude, longitude });
        }
        
        if !(0.0..=1.0).contains(&confidence) {
            return Err(GeographicError::InvalidConfidence(confidence));
        }
        
        let precision = layer.geohash_precision();
        let geohash = encode(geo::Coord { x: longitude, y: latitude }, precision)
            .map_err(|e| GeographicError::GeohashError(e.to_string()))?;
        
        Ok(Self {
            geohash,
            layer,
            confidence,
            timestamp: chrono::Utc::now(),
        })
    }
    
    /// Get the latitude and longitude from the geohash
    pub fn coordinates(&self) -> Result<(f64, f64), GeographicError> {
        let (coord, _, _) = decode(&self.geohash)
            .map_err(|e| GeographicError::GeohashError(e.to_string()))?;
        Ok((coord.y, coord.x))
    }
    
    /// Calculate distance to another geographic address in meters
    pub fn distance_to(&self, other: &GeographicAddress) -> Result<f64, GeographicError> {
        let (lat1, lon1) = self.coordinates()?;
        let (lat2, lon2) = other.coordinates()?;
        
        Ok(crate::utils::MathUtils::haversine_distance(lat1, lon1, lat2, lon2))
    }
}

/// A geographic location with multiple layers of precision
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GeographicLocation {
    /// The precise location
    pub coordinates: (f64, f64), // (latitude, longitude)
    /// Addresses for different geographic layers
    pub addresses: HashMap<GeographicLayer, GeographicAddress>,
    /// Location uncertainty in meters
    pub uncertainty_meters: f64,
    /// Timestamp when location was last updated
    pub last_updated: chrono::DateTime<chrono::Utc>,
}

impl GeographicLocation {
    /// Create a new geographic location
    pub fn new(latitude: f64, longitude: f64, uncertainty_meters: f64) -> Result<Self, GeographicError> {
        if !(-90.0..=90.0).contains(&latitude) || !(-180.0..=180.0).contains(&longitude) {
            return Err(GeographicError::InvalidCoordinates { latitude, longitude });
        }
        
        let mut addresses = HashMap::new();
        
        // Create addresses for all layers
        for layer in GeographicLayer::all_layers() {
            let address = GeographicAddress::new(latitude, longitude, layer, 1.0 - (uncertainty_meters / 1000.0))?;
            addresses.insert(layer, address);
        }
        
        Ok(Self {
            coordinates: (latitude, longitude),
            addresses,
            uncertainty_meters,
            last_updated: chrono::Utc::now(),
        })
    }
    
    /// Get the address for a specific geographic layer
    pub fn address_for_layer(&self, layer: GeographicLayer) -> Option<&GeographicAddress> {
        self.addresses.get(&layer)
    }
    
    /// Calculate distance to another location in meters
    pub fn distance_to(&self, other: &GeographicLocation) -> f64 {
        crate::utils::MathUtils::haversine_distance(
            self.coordinates.0, self.coordinates.1,
            other.coordinates.0, other.coordinates.1
        )
    }
    
    /// Check if this location is within a certain distance of another
    pub fn is_within_distance(&self, other: &GeographicLocation, distance_meters: f64) -> bool {
        self.distance_to(other) <= distance_meters
    }
    
    /// Get the appropriate geographic layer for a given distance
    pub fn layer_for_distance(&self, distance_meters: f64) -> GeographicLayer {
        for layer in GeographicLayer::all_layers() {
            if distance_meters <= layer.radius_meters() {
                return layer;
            }
        }
        GeographicLayer::Region
    }
}

impl std::fmt::Display for GeographicLocation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "({:.6}, {:.6}) ±{}m", 
               self.coordinates.0, 
               self.coordinates.1, 
               self.uncertainty_meters)
    }
}

/// Geographic sector for organizing routing tables
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GeographicSector {
    North,
    Northeast,
    East,
    Southeast,
    South,
    Southwest,
    West,
    Northwest,
}

impl GeographicSector {
    /// Get all sectors in order
    pub fn all_sectors() -> Vec<GeographicSector> {
        vec![
            GeographicSector::North,
            GeographicSector::Northeast,
            GeographicSector::East,
            GeographicSector::Southeast,
            GeographicSector::South,
            GeographicSector::Southwest,
            GeographicSector::West,
            GeographicSector::Northwest,
        ]
    }
    
    /// Determine which sector a location is in relative to a reference point
    pub fn from_relative_location(reference: &GeographicLocation, target: &GeographicLocation) -> GeographicSector {
        let (ref_lat, ref_lon) = reference.coordinates;
        let (target_lat, target_lon) = target.coordinates;
        
        let delta_lat = target_lat - ref_lat;
        let delta_lon = target_lon - ref_lon;
        
        // Determine sector based on angle
        let angle = delta_lon.atan2(delta_lat).to_degrees();
        
        match angle {
            a if a >= -22.5 && a < 22.5 => GeographicSector::North,
            a if a >= 22.5 && a < 67.5 => GeographicSector::Northeast,
            a if a >= 67.5 && a < 112.5 => GeographicSector::East,
            a if a >= 112.5 && a < 157.5 => GeographicSector::Southeast,
            a if a >= 157.5 || a < -157.5 => GeographicSector::South,
            a if a >= -157.5 && a < -112.5 => GeographicSector::Southwest,
            a if a >= -112.5 && a < -67.5 => GeographicSector::West,
            a if a >= -67.5 && a < -22.5 => GeographicSector::Northwest,
            _ => GeographicSector::North, // Default case
        }
    }
}

/// Node identity information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeIdentity {
    /// Unique node ID
    pub id: String,
    /// Node location
    pub location: GeographicLocation,
}

impl NodeIdentity {
    /// Generate a new node identity
    pub fn generate() -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            location: GeographicLocation::new(0.0, 0.0, 100.0).unwrap(),
        }
    }
}

/// Errors that can occur in geographic operations
#[derive(Error, Debug)]
pub enum GeographicError {
    #[error("Invalid coordinates: latitude={latitude}, longitude={longitude}")]
    InvalidCoordinates { latitude: f64, longitude: f64 },
    
    #[error("Invalid confidence value: {0} (must be between 0.0 and 1.0)")]
    InvalidConfidence(f64),
    
    #[error("Geohash error: {0}")]
    GeohashError(String),
    
    #[error("Distance calculation error: {0}")]
    DistanceError(String),
    
    #[error("Invalid geographic layer")]
    InvalidLayer,
    
    #[error("Location update failed: {0}")]
    LocationUpdateFailed(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_geographic_address_creation() {
        let address = GeographicAddress::new(37.7749, -122.4194, GeographicLayer::Neighborhood, 0.9).unwrap();
        assert_eq!(address.layer, GeographicLayer::Neighborhood);
        assert_eq!(address.confidence, 0.9);
        assert!(!address.geohash.is_empty());
    }
    
    #[test]
    fn test_geographic_location_creation() {
        let location = GeographicLocation::new(37.7749, -122.4194, 50.0).unwrap();
        assert_eq!(location.coordinates, (37.7749, -122.4194));
        assert_eq!(location.uncertainty_meters, 50.0);
        assert_eq!(location.addresses.len(), 5); // All layers
    }
    
    #[test]
    fn test_distance_calculation() {
        let loc1 = GeographicLocation::new(37.7749, -122.4194, 10.0).unwrap();
        let loc2 = GeographicLocation::new(37.7849, -122.4094, 10.0).unwrap();
        
        let distance = loc1.distance_to(&loc2);
        assert!(distance > 0.0);
        assert!(distance < 2000.0); // Should be less than 2km
    }
    
    #[test]
    fn test_geographic_sector_calculation() {
        let reference = GeographicLocation::new(37.7749, -122.4194, 10.0).unwrap();
        let north = GeographicLocation::new(37.7849, -122.4194, 10.0).unwrap();
        
        let sector = GeographicSector::from_relative_location(&reference, &north);
        assert_eq!(sector, GeographicSector::North);
    }
}