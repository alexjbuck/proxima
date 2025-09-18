//! Protocol definitions and message handling for Proxima
//!
//! This module defines the network protocols and message formats used
//! for communication between Proxima nodes.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::geo::*;
use crate::content::*;
use crate::network::*;
use crate::routing::*;

/// Protocol version
pub const PROTOCOL_VERSION: u32 = 1;

/// Protocol message types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProtocolMessage {
    /// Handshake message
    Handshake(HandshakeMessage),
    /// Keep-alive message
    KeepAlive(KeepAliveMessage),
    /// Content message
    Content(ContentProtocolMessage),
    /// Routing message
    Routing(RoutingProtocolMessage),
    /// Discovery message
    Discovery(DiscoveryProtocolMessage),
    /// Governance message
    Governance(GovernanceProtocolMessage),
    /// Error message
    Error(ErrorMessage),
}

/// Handshake message for initial connection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandshakeMessage {
    /// Protocol version
    pub version: u32,
    /// Node identity
    pub identity: NodeIdentity,
    /// Supported features
    pub features: Vec<String>,
    /// Challenge for authentication
    pub challenge: Vec<u8>,
}

/// Keep-alive message for connection maintenance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeepAliveMessage {
    /// Sender node ID
    pub sender: NodeId,
    /// Current location
    pub location: GeographicLocation,
    /// Node status
    pub status: NodeStatus,
    /// Timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Content protocol message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentProtocolMessage {
    /// Message type
    pub message_type: ContentMessageType,
    /// Content data
    pub content: Option<Content>,
    /// Content ID (for requests)
    pub content_id: Option<ContentId>,
    /// Geographic routing info
    pub routing_info: GeographicRoutingInfo,
    /// Message metadata
    pub metadata: HashMap<String, String>,
}

/// Content message types
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContentMessageType {
    /// Publish new content
    Publish,
    /// Request content
    Request,
    /// Content response
    Response,
    /// Content propagation
    Propagate,
    /// Content acknowledgment
    Acknowledge,
}

/// Routing protocol message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingProtocolMessage {
    /// Message type
    pub message_type: RoutingMessageType,
    /// Routing entries
    pub entries: Vec<RoutingEntry>,
    /// Target geographic region
    pub target_region: Option<GeographicAddress>,
    /// Message metadata
    pub metadata: HashMap<String, String>,
}

/// Routing message types
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RoutingMessageType {
    /// Route update
    Update,
    /// Route request
    Request,
    /// Route response
    Response,
    /// Route advertisement
    Advertisement,
}

/// Discovery protocol message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryProtocolMessage {
    /// Message type
    pub message_type: DiscoveryMessageType,
    /// Discovery query
    pub query: Option<DiscoveryQuery>,
    /// Discovery response
    pub response: Option<DiscoveryResponse>,
    /// Geographic region
    pub region: GeographicAddress,
    /// Message metadata
    pub metadata: HashMap<String, String>,
}

/// Discovery message types
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiscoveryMessageType {
    /// Discovery query
    Query,
    /// Discovery response
    Response,
    /// Node advertisement
    Advertisement,
    /// Node departure
    Departure,
}

/// Governance protocol message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernanceProtocolMessage {
    /// Message type
    pub message_type: GovernanceMessageType,
    /// Policy data
    pub policy: Option<GovernancePolicy>,
    /// Moderation action
    pub moderation_action: Option<ModerationAction>,
    /// Vote data
    pub vote: Option<PolicyVote>,
    /// Geographic community
    pub community: String,
    /// Message metadata
    pub metadata: HashMap<String, String>,
}

/// Governance message types
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GovernanceMessageType {
    /// Policy proposal
    PolicyProposal,
    /// Policy vote
    PolicyVote,
    /// Moderation action
    ModerationAction,
    /// Community update
    CommunityUpdate,
}

/// Error message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorMessage {
    /// Error code
    pub error_code: u32,
    /// Error message
    pub error_message: String,
    /// Error details
    pub error_details: Option<HashMap<String, String>>,
    /// Timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Protocol message builder
pub struct ProtocolMessageBuilder;

impl ProtocolMessageBuilder {
    /// Build handshake message
    pub fn handshake(identity: NodeIdentity, features: Vec<String>) -> ProtocolMessage {
        ProtocolMessage::Handshake(HandshakeMessage {
            version: PROTOCOL_VERSION,
            identity,
            features,
            challenge: vec![], // Would be generated in real implementation
        })
    }
    
    /// Build keep-alive message
    pub fn keep_alive(sender: NodeId, location: GeographicLocation, status: NodeStatus) -> ProtocolMessage {
        ProtocolMessage::KeepAlive(KeepAliveMessage {
            sender,
            location,
            status,
            timestamp: chrono::Utc::now(),
        })
    }
    
    /// Build content publish message
    pub fn content_publish(content: Content, routing_info: GeographicRoutingInfo) -> ProtocolMessage {
        ProtocolMessage::Content(ContentProtocolMessage {
            message_type: ContentMessageType::Publish,
            content: Some(content),
            content_id: None,
            routing_info,
            metadata: HashMap::new(),
        })
    }
    
    /// Build content request message
    pub fn content_request(content_id: ContentId, routing_info: GeographicRoutingInfo) -> ProtocolMessage {
        ProtocolMessage::Content(ContentProtocolMessage {
            message_type: ContentMessageType::Request,
            content: None,
            content_id: Some(content_id),
            routing_info,
            metadata: HashMap::new(),
        })
    }
    
    /// Build routing update message
    pub fn routing_update(entries: Vec<RoutingEntry>) -> ProtocolMessage {
        ProtocolMessage::Routing(RoutingProtocolMessage {
            message_type: RoutingMessageType::Update,
            entries,
            target_region: None,
            metadata: HashMap::new(),
        })
    }
    
    /// Build discovery query message
    pub fn discovery_query(query: DiscoveryQuery, region: GeographicAddress) -> ProtocolMessage {
        ProtocolMessage::Discovery(DiscoveryProtocolMessage {
            message_type: DiscoveryMessageType::Query,
            query: Some(query),
            response: None,
            region,
            metadata: HashMap::new(),
        })
    }
    
    /// Build governance policy proposal message
    pub fn governance_policy_proposal(policy: GovernancePolicy, community: String) -> ProtocolMessage {
        ProtocolMessage::Governance(GovernanceProtocolMessage {
            message_type: GovernanceMessageType::PolicyProposal,
            policy: Some(policy),
            moderation_action: None,
            vote: None,
            community,
            metadata: HashMap::new(),
        })
    }
    
    /// Build error message
    pub fn error(error_code: u32, error_message: String) -> ProtocolMessage {
        ProtocolMessage::Error(ErrorMessage {
            error_code,
            error_message,
            error_details: None,
            timestamp: chrono::Utc::now(),
        })
    }
}

/// Protocol message validator
pub struct ProtocolMessageValidator;

impl ProtocolMessageValidator {
    /// Validate protocol message
    pub fn validate(message: &ProtocolMessage) -> Result<(), ProtocolError> {
        match message {
            ProtocolMessage::Handshake(msg) => {
                if msg.version != PROTOCOL_VERSION {
                    return Err(ProtocolError::UnsupportedVersion(msg.version));
                }
                if msg.identity.id.0.is_empty() {
                    return Err(ProtocolError::InvalidIdentity);
                }
            }
            ProtocolMessage::KeepAlive(msg) => {
                if msg.sender.0.is_empty() {
                    return Err(ProtocolError::InvalidSender);
                }
            }
            ProtocolMessage::Content(msg) => {
                match msg.message_type {
                    ContentMessageType::Publish => {
                        if msg.content.is_none() {
                            return Err(ProtocolError::MissingContent);
                        }
                    }
                    ContentMessageType::Request => {
                        if msg.content_id.is_none() {
                            return Err(ProtocolError::MissingContentId);
                        }
                    }
                    _ => {}
                }
            }
            ProtocolMessage::Routing(msg) => {
                if msg.entries.is_empty() && msg.message_type == RoutingMessageType::Update {
                    return Err(ProtocolError::EmptyRoutingEntries);
                }
            }
            ProtocolMessage::Discovery(msg) => {
                match msg.message_type {
                    DiscoveryMessageType::Query => {
                        if msg.query.is_none() {
                            return Err(ProtocolError::MissingDiscoveryQuery);
                        }
                    }
                    DiscoveryMessageType::Response => {
                        if msg.response.is_none() {
                            return Err(ProtocolError::MissingDiscoveryResponse);
                        }
                    }
                    _ => {}
                }
            }
            ProtocolMessage::Governance(msg) => {
                match msg.message_type {
                    GovernanceMessageType::PolicyProposal => {
                        if msg.policy.is_none() {
                            return Err(ProtocolError::MissingPolicy);
                        }
                    }
                    GovernanceMessageType::ModerationAction => {
                        if msg.moderation_action.is_none() {
                            return Err(ProtocolError::MissingModerationAction);
                        }
                    }
                    _ => {}
                }
            }
            ProtocolMessage::Error(msg) => {
                if msg.error_message.is_empty() {
                    return Err(ProtocolError::EmptyErrorMessage);
                }
            }
        }
        
        Ok(())
    }
}

/// Protocol errors
#[derive(thiserror::Error, Debug)]
pub enum ProtocolError {
    #[error("Unsupported protocol version: {0}")]
    UnsupportedVersion(u32),
    
    #[error("Invalid node identity")]
    InvalidIdentity,
    
    #[error("Invalid sender")]
    InvalidSender,
    
    #[error("Missing content")]
    MissingContent,
    
    #[error("Missing content ID")]
    MissingContentId,
    
    #[error("Empty routing entries")]
    EmptyRoutingEntries,
    
    #[error("Missing discovery query")]
    MissingDiscoveryQuery,
    
    #[error("Missing discovery response")]
    MissingDiscoveryResponse,
    
    #[error("Missing policy")]
    MissingPolicy,
    
    #[error("Missing moderation action")]
    MissingModerationAction,
    
    #[error("Empty error message")]
    EmptyErrorMessage,
    
    #[error("Serialization error: {0}")]
    SerializationError(#[from] bincode::Error),
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_protocol_message_builder() {
        let identity = NodeIdentity::generate();
        let features = vec!["content_storage".to_string(), "routing".to_string()];
        
        let handshake = ProtocolMessageBuilder::handshake(identity, features);
        
        match handshake {
            ProtocolMessage::Handshake(msg) => {
                assert_eq!(msg.version, PROTOCOL_VERSION);
                assert!(!msg.identity.id.0.is_empty());
            }
            _ => panic!("Expected handshake message"),
        }
    }
    
    #[test]
    fn test_protocol_message_validation() {
        let identity = NodeIdentity::generate();
        let features = vec!["content_storage".to_string()];
        let handshake = ProtocolMessageBuilder::handshake(identity, features);
        
        assert!(ProtocolMessageValidator::validate(&handshake).is_ok());
    }
    
    #[test]
    fn test_protocol_message_validation_error() {
        let identity = NodeIdentity {
            id: NodeId::from_string("".to_string()),
            public_key: vec![],
            capabilities: NodeCapabilities {
                can_store: true,
                can_route: true,
                can_bridge: false,
                storage_capacity: 0,
                bandwidth_capacity: 0,
            },
            location: GeographicLocation::new(0.0, 0.0, 0.0).unwrap(),
            reputation: NodeReputation::default(),
        };
        let features = vec![];
        let handshake = ProtocolMessageBuilder::handshake(identity, features);
        
        assert!(ProtocolMessageValidator::validate(&handshake).is_err());
    }
}