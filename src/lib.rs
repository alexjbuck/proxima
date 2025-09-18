//! Proxima: A Decentralized Geographic Social Network

pub mod geo;
pub mod utils;
pub mod content;

pub use geo::*;
pub use utils::*;
pub use content::*;

/// Core error types for the Proxima network
#[derive(thiserror::Error, Debug)]
pub enum ProximaError {
    #[error("Geographic error: {0}")]
    Geographic(String),
    
    #[error("Content error: {0}")]
    Content(String),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Parse error: {0}")]
    Parse(#[from] std::num::ParseFloatError),
    
    #[error("Configuration error: {0}")]
    Config(String),
}

pub type Result<T> = std::result::Result<T, ProximaError>;

/// Main Proxima node implementation
pub struct ProximaNode {
    id: String,
    location: GeographicLocation,
    content_manager: ContentManager,
}

impl ProximaNode {
    /// Create a new Proxima node
    pub fn new(latitude: f64, longitude: f64) -> Result<Self> {
        let location = GeographicLocation::new(latitude, longitude, 50.0)
            .map_err(|e| ProximaError::Geographic(e.to_string()))?;
        
        let content_manager = ContentManager::new(1024 * 1024); // 1MB max content size
        
        Ok(Self {
            id: uuid::Uuid::new_v4().to_string(),
            location,
            content_manager,
        })
    }
    
    /// Get the node ID
    pub fn id(&self) -> &str {
        &self.id
    }
    
    /// Get the node location
    pub fn location(&self) -> &GeographicLocation {
        &self.location
    }
    
    /// Calculate distance to another node
    pub fn distance_to(&self, other: &ProximaNode) -> f64 {
        self.location.distance_to(&other.location)
    }
    
    /// Publish content
    pub fn publish_content(&mut self, content: Content) -> Result<ContentId> {
        self.content_manager.publish(content)
            .map_err(|e| ProximaError::Content(e.to_string()))
    }
    
    /// Get content by ID
    pub fn get_content(&self, content_id: &ContentId) -> Option<&Content> {
        self.content_manager.get(content_id)
    }
    
    /// Get all content
    pub fn get_all_content(&self) -> Vec<&Content> {
        self.content_manager.get_all()
    }
    
    /// Get content count
    pub fn content_count(&self) -> usize {
        self.content_manager.count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_node_creation() {
        let node = ProximaNode::new(37.7749, -122.4194).unwrap();
        assert!(!node.id().is_empty());
        assert_eq!(node.location().coordinates, (37.7749, -122.4194));
    }
    
    #[test]
    fn test_invalid_coordinates() {
        let result = ProximaNode::new(91.0, 0.0);
        assert!(result.is_err());
    }
    
    #[test]
    fn test_node_distance() {
        let node1 = ProximaNode::new(37.7749, -122.4194).unwrap();
        let node2 = ProximaNode::new(37.7849, -122.4094).unwrap();
        
        let distance = node1.distance_to(&node2);
        assert!(distance > 0.0);
        assert!(distance < 2000.0); // Should be less than 2km
    }
    
    #[test]
    fn test_content_publishing() {
        let mut node = ProximaNode::new(37.7749, -122.4194).unwrap();
        let location = GeographicLocation::new(37.7749, -122.4194, 10.0).unwrap();
        
        let content = Content::new(
            "test_user".to_string(),
            ContentType::Text,
            "Hello, Proxima!".as_bytes().to_vec(),
            location,
            vec!["test".to_string()],
        );
        
        let content_id = node.publish_content(content).unwrap();
        assert_eq!(node.content_count(), 1);
        
        let retrieved = node.get_content(&content_id).unwrap();
        assert_eq!(retrieved.author, "test_user");
        assert_eq!(retrieved.as_text(), Some("Hello, Proxima!".to_string()));
    }
}