//! Utility functions and helpers for Proxima

use std::time::Duration;
use serde::{Deserialize, Serialize};

/// Mathematical utilities for geographic calculations
pub struct MathUtils;

impl MathUtils {
    /// Calculate the great circle distance between two points using the Haversine formula
    pub fn haversine_distance(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
        const EARTH_RADIUS: f64 = 6_371_000.0; // Earth's radius in meters
        
        let dlat = (lat2 - lat1).to_radians();
        let dlon = (lon2 - lon1).to_radians();
        
        let a = (dlat / 2.0).sin().powi(2) + 
                lat1.to_radians().cos() * lat2.to_radians().cos() * 
                (dlon / 2.0).sin().powi(2);
        
        let c = 2.0 * a.sqrt().asin();
        
        EARTH_RADIUS * c
    }
    
    /// Calculate the bearing between two points
    pub fn bearing(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
        let dlon = (lon2 - lon1).to_radians();
        let lat1_rad = lat1.to_radians();
        let lat2_rad = lat2.to_radians();
        
        let y = dlon.sin() * lat2_rad.cos();
        let x = lat1_rad.cos() * lat2_rad.sin() - lat1_rad.sin() * lat2_rad.cos() * dlon.cos();
        
        let bearing = y.atan2(x).to_degrees();
        (bearing + 360.0) % 360.0
    }
    
    /// Calculate the midpoint between two geographic points
    pub fn midpoint(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> (f64, f64) {
        let dlon = (lon2 - lon1).to_radians();
        let lat1_rad = lat1.to_radians();
        let lat2_rad = lat2.to_radians();
        let lon1_rad = lon1.to_radians();
        
        let bx = lat2_rad.cos() * dlon.cos();
        let by = lat2_rad.cos() * dlon.sin();
        
        let lat3 = (lat1_rad.sin() + lat2_rad.sin()).atan2(
            ((lat1_rad.cos() + bx).powi(2) + by.powi(2)).sqrt()
        );
        
        let lon3 = lon1_rad + by.atan2(lat1_rad.cos() + bx);
        
        (lat3.to_degrees(), lon3.to_degrees())
    }
    
    /// Calculate the area of a geographic region (approximate)
    pub fn calculate_area(bounds: (f64, f64, f64, f64)) -> f64 {
        let (min_lat, min_lon, max_lat, max_lon) = bounds;
        
        // Simple approximation using average latitude
        let avg_lat = (min_lat + max_lat) / 2.0;
        let lat_factor = avg_lat.to_radians().cos();
        
        let lat_diff = (max_lat - min_lat).to_radians();
        let lon_diff = (max_lon - min_lon).to_radians();
        
        const EARTH_RADIUS: f64 = 6_371_000.0;
        
        lat_diff * lon_diff * lat_factor * EARTH_RADIUS * EARTH_RADIUS
    }
    
    /// Calculate the exponential decay factor
    pub fn exponential_decay(value: f64, half_life: f64, time: f64) -> f64 {
        (-time / half_life * std::f64::consts::LN_2).exp()
    }
    
    /// Calculate the sigmoid function
    pub fn sigmoid(x: f64) -> f64 {
        1.0 / (1.0 + (-x).exp())
    }
    
    /// Calculate the logistic function
    pub fn logistic(x: f64, k: f64, x0: f64) -> f64 {
        1.0 / (1.0 + (-k * (x - x0)).exp())
    }
}

/// Time utilities for temporal calculations
pub struct TimeUtils;

impl TimeUtils {
    /// Calculate the time difference in seconds
    pub fn time_diff_seconds(t1: chrono::DateTime<chrono::Utc>, t2: chrono::DateTime<chrono::Utc>) -> f64 {
        (t2 - t1).num_milliseconds() as f64 / 1000.0
    }
    
    /// Calculate the time difference in minutes
    pub fn time_diff_minutes(t1: chrono::DateTime<chrono::Utc>, t2: chrono::DateTime<chrono::Utc>) -> f64 {
        Self::time_diff_seconds(t1, t2) / 60.0
    }
    
    /// Calculate the time difference in hours
    pub fn time_diff_hours(t1: chrono::DateTime<chrono::Utc>, t2: chrono::DateTime<chrono::Utc>) -> f64 {
        Self::time_diff_seconds(t1, t2) / 3600.0
    }
    
    /// Calculate the time difference in days
    pub fn time_diff_days(t1: chrono::DateTime<chrono::Utc>, t2: chrono::DateTime<chrono::Utc>) -> f64 {
        Self::time_diff_seconds(t1, t2) / 86400.0
    }
    
    /// Get the current timestamp
    pub fn now() -> chrono::DateTime<chrono::Utc> {
        chrono::Utc::now()
    }
    
    /// Get the timestamp for a duration ago
    pub fn ago(duration: Duration) -> chrono::DateTime<chrono::Utc> {
        chrono::Utc::now() - chrono::Duration::from_std(duration).unwrap_or_default()
    }
    
    /// Get the timestamp for a duration from now
    pub fn from_now(duration: Duration) -> chrono::DateTime<chrono::Utc> {
        chrono::Utc::now() + chrono::Duration::from_std(duration).unwrap_or_default()
    }
}

/// String utilities for text processing
pub struct StringUtils;

impl StringUtils {
    /// Generate a random string of specified length
    pub fn random_string(length: usize) -> String {
        use rand::Rng;
        const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
        let mut rng = rand::thread_rng();
        
        (0..length)
            .map(|_| {
                let idx = rng.gen_range(0..CHARSET.len());
                CHARSET[idx] as char
            })
            .collect()
    }
    
    /// Truncate string to specified length with ellipsis
    pub fn truncate_with_ellipsis(s: &str, max_length: usize) -> String {
        if s.len() <= max_length {
            s.to_string()
        } else {
            format!("{}...", &s[..max_length.saturating_sub(3)])
        }
    }
    
    /// Check if string contains only alphanumeric characters
    pub fn is_alphanumeric(s: &str) -> bool {
        s.chars().all(|c| c.is_alphanumeric())
    }
    
    /// Check if string is a valid geohash
    pub fn is_valid_geohash(s: &str) -> bool {
        s.len() <= 12 && s.chars().all(|c| c.is_ascii_alphanumeric())
    }
}

/// Hash utilities for data integrity
pub struct HashUtils;

impl HashUtils {
    /// Calculate SHA-256 hash of data
    pub fn sha256(data: &[u8]) -> Vec<u8> {
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        hasher.update(data);
        hasher.finalize().to_vec()
    }
    
    /// Calculate simple hash for quick comparisons
    pub fn simple_hash(data: &[u8]) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let mut hasher = DefaultHasher::new();
        data.hash(&mut hasher);
        hasher.finish()
    }
    
    /// Verify hash integrity
    pub fn verify_hash(data: &[u8], expected_hash: &[u8]) -> bool {
        let actual_hash = Self::sha256(data);
        actual_hash == expected_hash
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_haversine_distance() {
        // Distance between San Francisco and New York (approximately 4,130 km)
        let distance = MathUtils::haversine_distance(37.7749, -122.4194, 40.7128, -74.0060);
        assert!((distance - 4_130_000.0).abs() < 100_000.0); // Within 100km tolerance
    }
    
    #[test]
    fn test_bearing_calculation() {
        let bearing = MathUtils::bearing(37.7749, -122.4194, 40.7128, -74.0060);
        assert!(bearing >= 0.0 && bearing <= 360.0);
    }
    
    #[test]
    fn test_midpoint_calculation() {
        let (lat, lon) = MathUtils::midpoint(37.7749, -122.4194, 40.7128, -74.0060);
        assert!(lat >= 37.7749 && lat <= 40.7128);
        assert!(lon >= -122.4194 && lon <= -74.0060);
    }
    
    #[test]
    fn test_exponential_decay() {
        let decay = MathUtils::exponential_decay(1.0, 1.0, 1.0);
        assert!((decay - 0.5).abs() < 0.01);
    }
    
    #[test]
    fn test_time_utils() {
        let now = TimeUtils::now();
        let past = TimeUtils::ago(Duration::from_secs(60));
        let diff = TimeUtils::time_diff_seconds(past, now);
        assert!((diff - 60.0).abs() < 1.0);
    }
    
    #[test]
    fn test_string_utils() {
        let random = StringUtils::random_string(10);
        assert_eq!(random.len(), 10);
        
        let truncated = StringUtils::truncate_with_ellipsis("Hello, world!", 5);
        assert_eq!(truncated, "He...");
        
        assert!(StringUtils::is_alphanumeric("abc123"));
        assert!(!StringUtils::is_alphanumeric("abc 123"));
    }
    
    #[test]
    fn test_hash_utils() {
        let data = b"Hello, world!";
        let hash = HashUtils::sha256(data);
        assert_eq!(hash.len(), 32); // SHA-256 produces 32 bytes
        
        assert!(HashUtils::verify_hash(data, &hash));
        assert!(!HashUtils::verify_hash(b"Different data", &hash));
    }
}