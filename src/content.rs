//! Content management for Proxima

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
}

impl Content {
    /// Create new content
    pub fn new(
        author: String,
        content_type: ContentType,
        data: Vec<u8>,
        location: GeographicLocation,
        tags: Vec<String>,
    ) -> Self {
        Self {
            id: ContentId::new(),
            author,
            content_type,
            data,
            timestamp: Utc::now(),
            location,
            tags,
        }
    }
    
    /// Get content size in bytes
    pub fn size(&self) -> usize {
        self.data.len()
    }
    
    /// Check if content is text
    pub fn is_text(&self) -> bool {
        matches!(self.content_type, ContentType::Text)
    }
    
    /// Get content as text (if it's text content)
    pub fn as_text(&self) -> Option<String> {
        if self.is_text() {
            String::from_utf8(self.data.clone()).ok()
        } else {
            None
        }
    }
}

/// Simple content manager
pub struct ContentManager {
    /// Content storage (in-memory for now)
    content: std::collections::HashMap<ContentId, Content>,
    /// Maximum content size
    max_content_size: usize,
}

impl ContentManager {
    /// Create a new content manager
    pub fn new(max_content_size: usize) -> Self {
        Self {
            content: std::collections::HashMap::new(),
            max_content_size,
        }
    }
    
    /// Publish content
    pub fn publish(&mut self, content: Content) -> Result<ContentId, ContentError> {
        // Validate content
        if content.size() > self.max_content_size {
            return Err(ContentError::ContentTooLarge {
                size: content.size(),
                max_size: self.max_content_size,
            });
        }
        
        if content.data.is_empty() {
            return Err(ContentError::EmptyContent);
        }
        
        let content_id = content.id.clone();
        self.content.insert(content_id.clone(), content);
        
        Ok(content_id)
    }
    
    /// Get content by ID
    pub fn get(&self, content_id: &ContentId) -> Option<&Content> {
        self.content.get(content_id)
    }
    
    /// Get all content
    pub fn get_all(&self) -> Vec<&Content> {
        self.content.values().collect()
    }
    
    /// Get content count
    pub fn count(&self) -> usize {
        self.content.len()
    }
    
    /// Remove content
    pub fn remove(&mut self, content_id: &ContentId) -> Option<Content> {
        self.content.remove(content_id)
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
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_content_creation() {
        let location = GeographicLocation::new(37.7749, -122.4194, 10.0).unwrap();
        let content = Content::new(
            "test_user".to_string(),
            ContentType::Text,
            "Hello, world!".as_bytes().to_vec(),
            location,
            vec!["test".to_string()],
        );
        
        assert_eq!(content.author, "test_user");
        assert_eq!(content.content_type, ContentType::Text);
        assert!(content.is_text());
        assert_eq!(content.as_text(), Some("Hello, world!".to_string()));
    }
    
    #[test]
    fn test_content_manager() {
        let mut manager = ContentManager::new(1024);
        let location = GeographicLocation::new(37.7749, -122.4194, 10.0).unwrap();
        
        let content = Content::new(
            "test_user".to_string(),
            ContentType::Text,
            "Hello, world!".as_bytes().to_vec(),
            location,
            vec![],
        );
        
        let content_id = manager.publish(content).unwrap();
        assert_eq!(manager.count(), 1);
        
        let retrieved = manager.get(&content_id).unwrap();
        assert_eq!(retrieved.author, "test_user");
    }
    
    #[test]
    fn test_content_too_large() {
        let mut manager = ContentManager::new(10); // Very small limit
        let location = GeographicLocation::new(37.7749, -122.4194, 10.0).unwrap();
        
        let content = Content::new(
            "test_user".to_string(),
            ContentType::Text,
            "This is a very long message that exceeds the size limit".as_bytes().to_vec(),
            location,
            vec![],
        );
        
        let result = manager.publish(content);
        assert!(result.is_err());
    }
}