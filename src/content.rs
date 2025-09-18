//! Content management and gravity model for Proxima

use std::collections::{HashMap, HashSet};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use thiserror::Error;

use crate::geo::*;

/// Unique identifier for content
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ContentId(pub String);

impl ContentId {
    /// Generate a new content ID
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }
    
    /// Create from string
    pub fn from_string(s: String) -> Self {
        Self(s)
    }
    
    /// Get as bytes for hashing
    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }
}

impl std::fmt::Display for ContentId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Types of content in the network
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ContentType {
    Text,
    Image,
    Video,
    Audio,
    Location,
    Event,
    Announcement,
    Emergency,
}

/// Content in the Proxima network
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Content {
    /// Unique identifier
    pub id: ContentId,
    /// Author of the content
    pub author: String,
    /// Type of content
    pub content_type: ContentType,
    /// Raw content data
    pub data: Vec<u8>,
    /// When the content was created
    pub timestamp: DateTime<Utc>,
    /// Geographic location where content was created
    pub location: GeographicLocation,
    /// Tags for categorization
    pub tags: Vec<String>,
    /// Content metadata
    pub metadata: ContentMetadata,
}

/// Metadata associated with content
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentMetadata {
    /// Geographic reach of this content (in meters)
    pub geographic_reach: f64,
    /// Half-life for temporal decay (in hours)
    pub half_life_hours: f64,
    /// Content mass (engagement weight)
    pub mass: f64,
    /// Social boost factor
    pub social_boost: f64,
    /// Content priority (higher = more important)
    pub priority: u8,
    /// Whether content is pinned to a location
    pub is_pinned: bool,
    /// Expiration time
    pub expires_at: Option<DateTime<Utc>>,
}

impl Default for ContentMetadata {
    fn default() -> Self {
        Self {
            geographic_reach: 1000.0, // 1km default
            half_life_hours: 24.0,    // 24 hours default
            mass: 1.0,                // Base mass
            social_boost: 1.0,        // No social boost initially
            priority: 5,              // Medium priority
            is_pinned: false,
            expires_at: None,
        }
    }
}

/// Content gravity metrics for propagation decisions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentGravity {
    /// Geographic relevance score
    pub geographic_relevance: f64,
    /// Social relevance score
    pub social_relevance: f64,
    /// Temporal relevance score
    pub temporal_relevance: f64,
    /// Combined relevance score
    pub combined_relevance: f64,
    /// Propagation probability
    pub propagation_probability: f64,
}

/// Content manager for handling content lifecycle
pub struct ContentManager {
    /// Content storage
    content_storage: std::collections::HashMap<ContentId, Content>,
    /// Content gravity calculator
    gravity_calculator: ContentGravityCalculator,
    /// Configuration
    config: ContentConfig,
}

/// Content gravity calculator
pub struct ContentGravityCalculator {
    /// Base geographic decay factor
    geographic_decay_factor: f64,
    /// Social boost multiplier
    social_boost_multiplier: f64,
    /// Temporal decay factor
    temporal_decay_factor: f64,
}

impl ContentGravityCalculator {
    /// Create a new gravity calculator
    pub fn new() -> Self {
        Self {
            geographic_decay_factor: 0.1,
            social_boost_multiplier: 1.5,
            temporal_decay_factor: 0.05,
        }
    }
    
    /// Calculate content gravity for a specific user location
    pub fn calculate_gravity(
        &self,
        content: &Content,
        user_location: &GeographicLocation,
        social_connections: &HashSet<String>,
    ) -> ContentGravity {
        // Geographic relevance
        let distance = content.location.distance_to(user_location);
        let geographic_relevance = self.calculate_geographic_relevance(
            &content.metadata,
            distance,
        );
        
        // Social relevance
        let social_relevance = self.calculate_social_relevance(
            content,
            social_connections,
        );
        
        // Temporal relevance
        let temporal_relevance = self.calculate_temporal_relevance(
            &content.metadata,
            content.timestamp,
        );
        
        // Combined relevance
        let combined_relevance = (
            geographic_relevance * 0.6 +
            social_relevance * 0.3 +
            temporal_relevance * 0.1
        ).min(1.0);
        
        // Propagation probability based on relevance
        let propagation_probability = (combined_relevance * 0.8 + 0.2).min(1.0);
        
        ContentGravity {
            geographic_relevance,
            social_relevance,
            temporal_relevance,
            combined_relevance,
            propagation_probability,
        }
    }
    
    /// Calculate geographic relevance based on distance
    fn calculate_geographic_relevance(
        &self,
        metadata: &ContentMetadata,
        distance_meters: f64,
    ) -> f64 {
        let reach = metadata.geographic_reach;
        let decay = self.geographic_decay_factor;
        
        // Exponential decay with distance
        (-distance_meters / reach * decay).exp()
    }
    
    /// Calculate social relevance based on connections
    fn calculate_social_relevance(
        &self,
        content: &Content,
        social_connections: &HashSet<String>,
    ) -> f64 {
        // For now, use a simple social boost
        let base_relevance = content.metadata.social_boost;
        let connection_boost = if social_connections.contains(&content.author) {
            self.social_boost_multiplier
        } else {
            1.0
        };
        
        (base_relevance * connection_boost).min(1.0)
    }
    
    /// Calculate temporal relevance based on age
    fn calculate_temporal_relevance(
        &self,
        metadata: &ContentMetadata,
        timestamp: DateTime<Utc>,
    ) -> f64 {
        let age_hours = (Utc::now() - timestamp).num_hours() as f64;
        let half_life = metadata.half_life_hours;
        
        // Exponential decay with time
        (-age_hours / half_life * self.temporal_decay_factor).exp()
    }
}

/// Content configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentConfig {
    /// Maximum content size in bytes
    pub max_content_size: usize,
    /// Default content TTL
    pub default_ttl: std::time::Duration,
    /// Maximum number of active waves
    pub max_active_waves: usize,
    /// Wave propagation speed (meters per second)
    pub wave_speed: f64,
    /// Minimum relevance threshold for propagation
    pub min_relevance_threshold: f64,
}

impl Default for ContentConfig {
    fn default() -> Self {
        Self {
            max_content_size: 1024 * 1024, // 1MB
            default_ttl: std::time::Duration::from_secs(86400), // 24 hours
            max_active_waves: 1000,
            wave_speed: 100.0, // 100 m/s (roughly network propagation speed)
            min_relevance_threshold: 0.1,
        }
    }
}

impl ContentManager {
    /// Create a new content manager
    pub fn new(config: ContentConfig) -> Self {
        Self {
            content_storage: std::collections::HashMap::new(),
            gravity_calculator: ContentGravityCalculator::new(),
            config,
        }
    }
    
    /// Start the content manager
    pub async fn start(&self) -> Result<(), ContentError> {
        tracing::info!("Content manager started");
        Ok(())
    }
    
    /// Publish new content
    pub async fn publish(&self, content: Content) -> Result<ContentId, ContentError> {
        // Validate content
        self.validate_content(&content)?;
        
        // Store content (in a real implementation, this would be persistent)
        let content_id = content.id.clone();
        
        tracing::info!("Published content: {}", content_id);
        
        Ok(content_id)
    }
    
    /// Get content relevant to a geographic location
    pub async fn get_geographic_content(
        &self,
        location: &GeographicLocation,
        radius_meters: f64,
    ) -> Result<Vec<Content>, ContentError> {
        let mut relevant_content = Vec::new();
        
        // In a real implementation, this would query a spatial index
        // For now, return empty vector
        tracing::debug!("Getting content for location {} within {}m", location, radius_meters);
        
        Ok(relevant_content)
    }
    
    /// Validate content before publishing
    fn validate_content(&self, content: &Content) -> Result<(), ContentError> {
        if content.data.len() > self.config.max_content_size {
            return Err(ContentError::ContentTooLarge {
                size: content.data.len(),
                max_size: self.config.max_content_size,
            });
        }
        
        if content.data.is_empty() {
            return Err(ContentError::EmptyContent);
        }
        
        Ok(())
    }
}

/// Content errors
#[derive(Error, Debug)]
pub enum ContentError {
    #[error("Content too large: {size} bytes (max: {max_size})")]
    ContentTooLarge { size: usize, max_size: usize },
    
    #[error("Empty content")]
    EmptyContent,
    
    #[error("Geographic error: {0}")]
    GeographicError(#[from] GeographicError),
    
    #[error("Content propagation failed: {0}")]
    PropagationFailed(String),
    
    #[error("Content validation failed: {0}")]
    ValidationFailed(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_content_creation() {
        let location = GeographicLocation::new(37.7749, -122.4194, 10.0).unwrap();
        let content = Content {
            id: ContentId::new(),
            author: "test_user".to_string(),
            content_type: ContentType::Text,
            data: "Hello, world!".as_bytes().to_vec(),
            timestamp: Utc::now(),
            location: location.clone(),
            tags: vec!["test".to_string()],
            metadata: ContentMetadata::default(),
        };
        
        assert_eq!(content.author, "test_user");
        assert_eq!(content.content_type, ContentType::Text);
    }
    
    #[test]
    fn test_content_gravity_calculation() {
        let calculator = ContentGravityCalculator::new();
        let location = GeographicLocation::new(37.7749, -122.4194, 10.0).unwrap();
        let content = Content {
            id: ContentId::new(),
            author: "test_user".to_string(),
            content_type: ContentType::Text,
            data: "Hello, world!".as_bytes().to_vec(),
            timestamp: Utc::now(),
            location: location.clone(),
            tags: vec![],
            metadata: ContentMetadata::default(),
        };
        
        let gravity = calculator.calculate_gravity(&content, &location, &HashSet::new());
        
        assert!(gravity.geographic_relevance > 0.0);
        assert!(gravity.combined_relevance > 0.0);
        assert!(gravity.propagation_probability > 0.0);
    }
}