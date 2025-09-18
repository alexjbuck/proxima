//! Privacy and security mechanisms for Proxima
//!
//! This module implements geographic privacy through ambiguity and security
//! mechanisms to protect user location and content.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};
use serde::{Deserialize, Serialize};
use ring::digest;
use ed25519_dalek::{Keypair, PublicKey, SecretKey, Signature, Signer, Verifier};
use thiserror::Error;

use crate::geo::*;
use crate::content::*;

/// Privacy manager for handling location privacy and security
pub struct PrivacyManager {
    /// Privacy configuration
    config: PrivacyConfig,
    /// Location mixing pool
    mixing_pool: Arc<tokio::sync::RwLock<LocationMixingPool>>,
    /// Anonymity set tracker
    anonymity_tracker: Arc<tokio::sync::RwLock<AnonymityTracker>>,
    /// Cryptographic keypair
    keypair: Keypair,
}

/// Location mixing pool for privacy
#[derive(Debug, Clone)]
pub struct LocationMixingPool {
    /// Pooled location updates
    location_updates: VecDeque<LocationUpdate>,
    /// Maximum pool size
    max_pool_size: usize,
    /// Mixing delay
    mixing_delay: Duration,
}

/// Location update for mixing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocationUpdate {
    /// Node ID (encrypted)
    pub encrypted_node_id: Vec<u8>,
    /// Location (with precision adjustment)
    pub location: GeographicLocation,
    /// Timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Mixing group ID
    pub mixing_group_id: String,
}

/// Anonymity tracker for k-anonymity
#[derive(Debug, Clone)]
pub struct AnonymityTracker {
    /// Geographic anonymity sets
    anonymity_sets: HashMap<String, AnonymitySet>, // geohash -> set
    /// Minimum anonymity threshold
    min_anonymity: usize,
}

/// Anonymity set for a geographic region
#[derive(Debug, Clone)]
pub struct AnonymitySet {
    /// Users in this anonymity set
    users: HashSet<String>,
    /// Geographic region
    region: GeographicRegion,
    /// Last update
    last_update: chrono::DateTime<chrono::Utc>,
}

/// Privacy configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivacyConfig {
    /// Location precision in meters
    pub location_precision: f64,
    /// K-anonymity threshold
    pub k_anonymity: usize,
    /// Enable location mixing
    pub enable_mixing: bool,
    /// Mixing delay in seconds
    pub mixing_delay_seconds: u64,
    /// Maximum mixing pool size
    pub max_mixing_pool_size: usize,
    /// Enable geographic obfuscation
    pub enable_obfuscation: bool,
    /// Obfuscation radius in meters
    pub obfuscation_radius: f64,
}

impl Default for PrivacyConfig {
    fn default() -> Self {
        Self {
            location_precision: 100.0, // 100m precision
            k_anonymity: 5,            // 5-anonymity
            enable_mixing: true,
            mixing_delay_seconds: 60,  // 1 minute mixing delay
            max_mixing_pool_size: 100,
            enable_obfuscation: true,
            obfuscation_radius: 50.0,  // 50m obfuscation
        }
    }
}

/// Privacy manager implementation
impl PrivacyManager {
    /// Create a new privacy manager
    pub fn new(config: PrivacyConfig) -> Self {
        let keypair = Keypair::generate(&mut rand::rngs::OsRng);
        
        Self {
            config,
            mixing_pool: Arc::new(tokio::sync::RwLock::new(LocationMixingPool {
                location_updates: VecDeque::new(),
                max_pool_size: 100,
                mixing_delay: Duration::from_secs(60),
            })),
            anonymity_tracker: Arc::new(tokio::sync::RwLock::new(AnonymityTracker {
                anonymity_sets: HashMap::new(),
                min_anonymity: 5,
            })),
            keypair,
        }
    }
    
    /// Adjust location precision based on privacy requirements
    pub fn adjust_location_precision(
        &self,
        location: &GeographicLocation,
        user_density: usize,
    ) -> Result<GeographicLocation, PrivacyError> {
        let mut adjusted_location = location.clone();
        
        // Calculate required precision based on k-anonymity
        let required_precision = self.calculate_required_precision(user_density)?;
        
        // Adjust precision if needed
        if required_precision > self.config.location_precision {
            adjusted_location.uncertainty_meters = required_precision;
            
            // Update addresses with new precision
            adjusted_location.addresses.clear();
            for layer in GeographicLayer::all_layers() {
                let confidence = 1.0 - (required_precision / 1000.0).min(1.0);
                let address = GeographicAddress::new(
                    location.coordinates.0,
                    location.coordinates.1,
                    layer,
                    confidence,
                )?;
                adjusted_location.addresses.insert(layer, address);
            }
        }
        
        Ok(adjusted_location)
    }
    
    /// Add location update to mixing pool
    pub async fn add_location_update(
        &self,
        node_id: &str,
        location: GeographicLocation,
    ) -> Result<(), PrivacyError> {
        let mut pool = self.mixing_pool.write().await;
        
        // Encrypt node ID
        let encrypted_node_id = self.encrypt_node_id(node_id)?;
        
        // Create location update
        let update = LocationUpdate {
            encrypted_node_id,
            location: self.obfuscate_location(&location)?,
            timestamp: chrono::Utc::now(),
            mixing_group_id: self.generate_mixing_group_id(&location)?,
        };
        
        // Add to pool
        pool.location_updates.push_back(update);
        
        // Limit pool size
        if pool.location_updates.len() > pool.max_pool_size {
            pool.location_updates.pop_front();
        }
        
        Ok(())
    }
    
    /// Process mixing pool and release updates
    pub async fn process_mixing_pool(&self) -> Result<Vec<LocationUpdate>, PrivacyError> {
        let mut pool = self.mixing_pool.write().await;
        let mut ready_updates = Vec::new();
        
        let now = chrono::Utc::now();
        
        // Find updates ready for release
        while let Some(update) = pool.location_updates.front() {
            if (now - update.timestamp).to_std().unwrap_or(Duration::ZERO) >= pool.mixing_delay {
                ready_updates.push(pool.location_updates.pop_front().unwrap());
            } else {
                break;
            }
        }
        
        // Shuffle updates for additional privacy
        use rand::seq::SliceRandom;
        let mut rng = rand::thread_rng();
        ready_updates.shuffle(&mut rng);
        
        Ok(ready_updates)
    }
    
    /// Check k-anonymity for a location
    pub async fn check_k_anonymity(
        &self,
        location: &GeographicLocation,
    ) -> Result<bool, PrivacyError> {
        let tracker = self.anonymity_tracker.read().await;
        
        // Get anonymity set for this location
        let geohash = location.address_for_layer(GeographicLayer::Neighborhood)
            .map(|addr| addr.geohash.clone())
            .unwrap_or_else(|| "unknown".to_string());
        
        if let Some(anonymity_set) = tracker.anonymity_sets.get(&geohash) {
            Ok(anonymity_set.users.len() >= self.config.k_anonymity)
        } else {
            Ok(false)
        }
    }
    
    /// Update anonymity set
    pub async fn update_anonymity_set(
        &self,
        node_id: &str,
        location: &GeographicLocation,
    ) -> Result<(), PrivacyError> {
        let mut tracker = self.anonymity_tracker.write().await;
        
        let geohash = location.address_for_layer(GeographicLayer::Neighborhood)
            .map(|addr| addr.geohash.clone())
            .unwrap_or_else(|| "unknown".to_string());
        
        let anonymity_set = tracker.anonymity_sets.entry(geohash.clone()).or_insert_with(|| {
            AnonymitySet {
                users: HashSet::new(),
                region: GeographicRegion {
                    bounds: (-90.0, -180.0, 90.0, 180.0), // Will be updated
                    activity_level: 0.0,
                    content_count: 0,
                },
                last_update: chrono::Utc::now(),
            }
        });
        
        anonymity_set.users.insert(node_id.to_string());
        anonymity_set.last_update = chrono::Utc::now();
        
        Ok(())
    }
    
    /// Sign content for authenticity
    pub fn sign_content(&self, content: &Content) -> Result<Signature, PrivacyError> {
        let content_hash = self.hash_content(content)?;
        Ok(self.keypair.sign(&content_hash))
    }
    
    /// Verify content signature
    pub fn verify_content_signature(
        &self,
        content: &Content,
        signature: &Signature,
        public_key: &PublicKey,
    ) -> Result<bool, PrivacyError> {
        let content_hash = self.hash_content(content)?;
        Ok(public_key.verify(&content_hash, signature).is_ok())
    }
    
    /// Calculate required precision for k-anonymity
    fn calculate_required_precision(&self, user_density: usize) -> Result<f64, PrivacyError> {
        if user_density < self.config.k_anonymity {
            // Need to reduce precision to achieve k-anonymity
            let precision_factor = (self.config.k_anonymity as f64) / (user_density as f64);
            Ok(self.config.location_precision * precision_factor)
        } else {
            Ok(self.config.location_precision)
        }
    }
    
    /// Encrypt node ID for privacy
    fn encrypt_node_id(&self, node_id: &str) -> Result<Vec<u8>, PrivacyError> {
        // Simple encryption using hash (in real implementation, use proper encryption)
        let hash = digest::digest(&digest::SHA256, node_id.as_bytes());
        Ok(hash.as_ref().to_vec())
    }
    
    /// Obfuscate location for privacy
    fn obfuscate_location(&self, location: &GeographicLocation) -> Result<GeographicLocation, PrivacyError> {
        if !self.config.enable_obfuscation {
            return Ok(location.clone());
        }
        
        let mut obfuscated = location.clone();
        
        // Add random offset within obfuscation radius
        let offset_lat = (rand::random::<f64>() - 0.5) * 2.0 * self.config.obfuscation_radius / 111_000.0;
        let offset_lon = (rand::random::<f64>() - 0.5) * 2.0 * self.config.obfuscation_radius / 111_000.0;
        
        obfuscated.coordinates.0 += offset_lat;
        obfuscated.coordinates.1 += offset_lon;
        
        // Update addresses
        obfuscated.addresses.clear();
        for layer in GeographicLayer::all_layers() {
            let confidence = 1.0 - (self.config.obfuscation_radius / 1000.0).min(1.0);
            let address = GeographicAddress::new(
                obfuscated.coordinates.0,
                obfuscated.coordinates.1,
                layer,
                confidence,
            )?;
            obfuscated.addresses.insert(layer, address);
        }
        
        Ok(obfuscated)
    }
    
    /// Generate mixing group ID
    fn generate_mixing_group_id(&self, location: &GeographicLocation) -> Result<String, PrivacyError> {
        let geohash = location.address_for_layer(GeographicLayer::Neighborhood)
            .map(|addr| addr.geohash.clone())
            .unwrap_or_else(|| "unknown".to_string());
        
        // Create time-based group ID
        let time_bucket = chrono::Utc::now().timestamp() / 300; // 5-minute buckets
        Ok(format!("{}_{}", geohash, time_bucket))
    }
    
    /// Hash content for signing
    fn hash_content(&self, content: &Content) -> Result<Vec<u8>, PrivacyError> {
        let serialized = bincode::serialize(content)?;
        let hash = digest::digest(&digest::SHA256, &serialized);
        Ok(hash.as_ref().to_vec())
    }
}

/// Privacy errors
#[derive(Error, Debug)]
pub enum PrivacyError {
    #[error("Geographic error: {0}")]
    GeographicError(#[from] GeographicError),
    
    #[error("Serialization error: {0}")]
    SerializationError(#[from] bincode::Error),
    
    #[error("Cryptographic error: {0}")]
    CryptographicError(String),
    
    #[error("Privacy violation: {0}")]
    PrivacyViolation(String),
    
    #[error("Anonymity threshold not met")]
    AnonymityThresholdNotMet,
    
    #[error("Location obfuscation failed: {0}")]
    LocationObfuscationFailed(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_privacy_manager_creation() {
        let config = PrivacyConfig::default();
        let privacy_manager = PrivacyManager::new(config);
        assert!(privacy_manager.keypair.public_key().as_bytes().len() > 0);
    }
    
    #[test]
    fn test_location_precision_adjustment() {
        let config = PrivacyConfig::default();
        let privacy_manager = PrivacyManager::new(config);
        let location = GeographicLocation::new(37.7749, -122.4194, 10.0).unwrap();
        
        let adjusted = privacy_manager.adjust_location_precision(&location, 3).unwrap();
        assert!(adjusted.uncertainty_meters >= location.uncertainty_meters);
    }
    
    #[tokio::test]
    async fn test_location_mixing() {
        let config = PrivacyConfig::default();
        let privacy_manager = PrivacyManager::new(config);
        let location = GeographicLocation::new(37.7749, -122.4194, 10.0).unwrap();
        
        privacy_manager.add_location_update("test_node", location).await.unwrap();
        
        let updates = privacy_manager.process_mixing_pool().await.unwrap();
        assert_eq!(updates.len(), 0); // No updates ready yet
    }
}