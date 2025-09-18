//! Proxima: A Decentralized Geographic Social Network

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
    location: (f64, f64), // (latitude, longitude)
}

impl ProximaNode {
    /// Create a new Proxima node
    pub fn new(latitude: f64, longitude: f64) -> Result<Self> {
        if !(-90.0..=90.0).contains(&latitude) || !(-180.0..=180.0).contains(&longitude) {
            return Err(ProximaError::Geographic("Invalid coordinates".to_string()));
        }
        
        Ok(Self {
            id: uuid::Uuid::new_v4().to_string(),
            location: (latitude, longitude),
        })
    }
    
    /// Get the node ID
    pub fn id(&self) -> &str {
        &self.id
    }
    
    /// Get the node location
    pub fn location(&self) -> (f64, f64) {
        self.location
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_node_creation() {
        let node = ProximaNode::new(37.7749, -122.4194).unwrap();
        assert!(!node.id().is_empty());
        assert_eq!(node.location(), (37.7749, -122.4194));
    }
    
    #[test]
    fn test_invalid_coordinates() {
        let result = ProximaNode::new(91.0, 0.0);
        assert!(result.is_err());
    }
}