//! Proxima: A Decentralized Geographic Social Network
//!
//! This library implements a fundamentally geographic social network where physical location
//! forms the primary organizing principle of the network topology.

pub mod geo;
pub mod content;
pub mod utils;

pub use geo::*;
pub use content::*;
pub use utils::*;

/// Core error types for the Proxima network
#[derive(thiserror::Error, Debug)]
pub enum ProximaError {
    #[error("Geographic error: {0}")]
    Geographic(#[from] geo::GeographicError),
    
    #[error("Content error: {0}")]
    Content(#[from] content::ContentError),
    
    #[error("Serialization error: {0}")]
    Serialization(#[from] bincode::Error),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Configuration error: {0}")]
    Config(String),
}

pub type Result<T> = std::result::Result<T, ProximaError>;

/// Main Proxima node implementation
pub struct ProximaNode {
    identity: NodeIdentity,
    location: GeographicLocation,
    content_manager: ContentManager,
}

impl ProximaNode {
    /// Create a new Proxima node
    pub async fn new(
        location: GeographicLocation,
        config: NodeConfig,
    ) -> Result<Self> {
        let identity = NodeIdentity::generate();
        let content_manager = ContentManager::new(config.content.clone());
        
        Ok(Self {
            identity,
            location,
            content_manager,
        })
    }
    
    /// Start the node and begin participating in the network
    pub async fn start(&mut self) -> Result<()> {
        // Start content management
        self.content_manager.start().await?;
        
        tracing::info!(
            "Proxima node started at location: {}",
            self.location
        );
        
        Ok(())
    }
    
    /// Publish content to the network
    pub async fn publish_content(&mut self, content: Content) -> Result<ContentId> {
        let content_id = self.content_manager.publish(content).await?;
        Ok(content_id)
    }
    
    /// Get content relevant to current location
    pub async fn get_local_content(&self, radius: f64) -> Result<Vec<Content>> {
        self.content_manager.get_geographic_content(&self.location, radius).await
    }
}

/// Configuration for a Proxima node
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NodeConfig {
    pub content: ContentConfig,
}

impl Default for NodeConfig {
    fn default() -> Self {
        Self {
            content: ContentConfig::default(),
        }
    }
}

/// Load configuration from file
impl NodeConfig {
    pub fn load_from_file(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        let config: NodeConfig = toml::from_str(&content)?;
        Ok(config)
    }
}