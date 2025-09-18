//! Geographic governance and moderation for Proxima
//!
//! This module implements the governance mechanisms that allow geographic
//! communities to self-moderate and make decisions about local content and policies.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};
use serde::{Deserialize, Serialize};
use tokio::sync::{RwLock, mpsc};
use thiserror::Error;

use crate::geo::*;
use crate::content::*;
use crate::network::*;

/// Geographic governance manager
pub struct GovernanceManager {
    /// Geographic communities
    communities: Arc<RwLock<HashMap<String, GeographicCommunity>>>, // geohash -> community
    /// Governance policies
    policies: Arc<RwLock<HashMap<String, GovernancePolicy>>>, // policy_id -> policy
    /// Moderation actions
    moderation_actions: Arc<RwLock<VecDeque<ModerationAction>>>,
    /// Governance configuration
    config: GovernanceConfig,
}

/// Geographic community
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeographicCommunity {
    /// Community ID (geohash)
    pub id: String,
    /// Community geographic region
    pub region: GeographicRegion,
    /// Community members
    pub members: HashSet<NodeId>,
    /// Community moderators
    pub moderators: HashSet<NodeId>,
    /// Community policies
    pub policies: Vec<String>, // policy IDs
    /// Community reputation
    pub reputation: f64,
    /// Last activity
    pub last_activity: chrono::DateTime<chrono::Utc>,
}

/// Governance policy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernancePolicy {
    /// Policy ID
    pub id: String,
    /// Policy name
    pub name: String,
    /// Policy description
    pub description: String,
    /// Policy type
    pub policy_type: PolicyType,
    /// Geographic scope
    pub geographic_scope: GeographicRegion,
    /// Policy rules
    pub rules: Vec<PolicyRule>,
    /// Policy creator
    pub creator: NodeId,
    /// Policy creation time
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Policy status
    pub status: PolicyStatus,
}

/// Policy types
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PolicyType {
    /// Content moderation policy
    ContentModeration,
    /// Community standards policy
    CommunityStandards,
    /// Emergency response policy
    EmergencyResponse,
    /// Resource allocation policy
    ResourceAllocation,
    /// Privacy policy
    Privacy,
}

/// Policy status
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PolicyStatus {
    /// Policy is active
    Active,
    /// Policy is pending approval
    Pending,
    /// Policy is rejected
    Rejected,
    /// Policy is expired
    Expired,
}

/// Policy rule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyRule {
    /// Rule ID
    pub id: String,
    /// Rule condition
    pub condition: RuleCondition,
    /// Rule action
    pub action: RuleAction,
    /// Rule priority
    pub priority: u8,
}

/// Rule conditions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RuleCondition {
    /// Content contains specific keywords
    ContainsKeywords(Vec<String>),
    /// Content is from specific author
    FromAuthor(NodeId),
    /// Content is in specific geographic region
    InGeographicRegion(GeographicRegion),
    /// Content has specific type
    HasContentType(ContentType),
    /// Content violates community standards
    ViolatesCommunityStandards,
    /// Emergency condition
    EmergencyCondition,
}

/// Rule actions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RuleAction {
    /// Allow content
    Allow,
    /// Block content
    Block,
    /// Flag content for review
    FlagForReview,
    /// Reduce content visibility
    ReduceVisibility(f64), // visibility factor (0.0 to 1.0)
    /// Require additional verification
    RequireVerification,
    /// Emergency broadcast
    EmergencyBroadcast,
}

/// Moderation action
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModerationAction {
    /// Action ID
    pub id: String,
    /// Action type
    pub action_type: ModerationActionType,
    /// Target content
    pub target_content: ContentId,
    /// Moderator
    pub moderator: NodeId,
    /// Geographic community
    pub community: String,
    /// Action reason
    pub reason: String,
    /// Action timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Action status
    pub status: ModerationStatus,
}

/// Moderation action types
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModerationActionType {
    /// Approve content
    Approve,
    /// Reject content
    Reject,
    /// Flag content
    Flag,
    /// Remove content
    Remove,
    /// Restrict content
    Restrict,
    /// Warn user
    Warn,
}

/// Moderation status
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModerationStatus {
    /// Action is pending
    Pending,
    /// Action is approved
    Approved,
    /// Action is rejected
    Rejected,
    /// Action is completed
    Completed,
}

/// Governance configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernanceConfig {
    /// Minimum community size for governance
    pub min_community_size: usize,
    /// Moderation quorum size
    pub moderation_quorum: usize,
    /// Policy voting period
    pub policy_voting_period: Duration,
    /// Moderation timeout
    pub moderation_timeout: Duration,
    /// Enable emergency protocols
    pub enable_emergency_protocols: bool,
    /// Geographic sovereignty enabled
    pub enable_geographic_sovereignty: bool,
}

impl Default for GovernanceConfig {
    fn default() -> Self {
        Self {
            min_community_size: 10,
            moderation_quorum: 3,
            policy_voting_period: Duration::from_secs(86400), // 24 hours
            moderation_timeout: Duration::from_secs(3600), // 1 hour
            enable_emergency_protocols: true,
            enable_geographic_sovereignty: true,
        }
    }
}

/// Governance manager implementation
impl GovernanceManager {
    /// Create a new governance manager
    pub fn new(config: GovernanceConfig) -> Self {
        Self {
            communities: Arc::new(RwLock::new(HashMap::new())),
            policies: Arc::new(RwLock::new(HashMap::new())),
            moderation_actions: Arc::new(RwLock::new(VecDeque::new())),
            config,
        }
    }
    
    /// Start governance manager
    pub async fn start(&self) -> Result<(), GovernanceError> {
        // Start policy enforcement
        self.start_policy_enforcement().await?;
        
        // Start moderation processing
        self.start_moderation_processing().await?;
        
        // Start community management
        self.start_community_management().await?;
        
        Ok(())
    }
    
    /// Create a new geographic community
    pub async fn create_community(
        &self,
        region: GeographicRegion,
        creator: NodeId,
    ) -> Result<String, GovernanceError> {
        let community_id = self.generate_community_id(&region);
        
        let community = GeographicCommunity {
            id: community_id.clone(),
            region,
            members: HashSet::from([creator.clone()]),
            moderators: HashSet::from([creator]),
            policies: Vec::new(),
            reputation: 0.5, // Neutral reputation
            last_activity: chrono::Utc::now(),
        };
        
        let mut communities = self.communities.write().await;
        communities.insert(community_id.clone(), community);
        
        Ok(community_id)
    }
    
    /// Join a geographic community
    pub async fn join_community(
        &self,
        community_id: &str,
        node_id: NodeId,
    ) -> Result<(), GovernanceError> {
        let mut communities = self.communities.write().await;
        
        if let Some(community) = communities.get_mut(community_id) {
            community.members.insert(node_id);
            community.last_activity = chrono::Utc::now();
        } else {
            return Err(GovernanceError::CommunityNotFound(community_id.to_string()));
        }
        
        Ok(())
    }
    
    /// Create a new governance policy
    pub async fn create_policy(
        &self,
        name: String,
        description: String,
        policy_type: PolicyType,
        geographic_scope: GeographicRegion,
        rules: Vec<PolicyRule>,
        creator: NodeId,
    ) -> Result<String, GovernanceError> {
        let policy_id = uuid::Uuid::new_v4().to_string();
        
        let policy = GovernancePolicy {
            id: policy_id.clone(),
            name,
            description,
            policy_type,
            geographic_scope,
            rules,
            creator,
            created_at: chrono::Utc::now(),
            status: PolicyStatus::Pending,
        };
        
        let mut policies = self.policies.write().await;
        policies.insert(policy_id.clone(), policy);
        
        Ok(policy_id)
    }
    
    /// Vote on a policy
    pub async fn vote_on_policy(
        &self,
        policy_id: &str,
        voter: NodeId,
        vote: PolicyVote,
    ) -> Result<(), GovernanceError> {
        let mut policies = self.policies.write().await;
        
        if let Some(policy) = policies.get_mut(policy_id) {
            // Check if voter is in the geographic scope
            if !self.is_node_in_geographic_scope(voter, &policy.geographic_scope).await? {
                return Err(GovernanceError::VoterNotInScope);
            }
            
            // Process vote (simplified implementation)
            match vote {
                PolicyVote::Approve => {
                    // In a real implementation, this would track votes and check quorum
                    policy.status = PolicyStatus::Active;
                }
                PolicyVote::Reject => {
                    policy.status = PolicyStatus::Rejected;
                }
            }
        } else {
            return Err(GovernanceError::PolicyNotFound(policy_id.to_string()));
        }
        
        Ok(())
    }
    
    /// Moderate content
    pub async fn moderate_content(
        &self,
        content_id: ContentId,
        moderator: NodeId,
        action_type: ModerationActionType,
        reason: String,
    ) -> Result<String, GovernanceError> {
        // Find the appropriate community for this content
        let community_id = self.find_community_for_content(&content_id).await?;
        
        // Check if moderator has permission
        if !self.has_moderation_permission(&community_id, &moderator).await? {
            return Err(GovernanceError::InsufficientPermissions);
        }
        
        let action_id = uuid::Uuid::new_v4().to_string();
        
        let action = ModerationAction {
            id: action_id.clone(),
            action_type,
            target_content: content_id,
            moderator,
            community: community_id,
            reason,
            timestamp: chrono::Utc::now(),
            status: ModerationStatus::Pending,
        };
        
        let mut actions = self.moderation_actions.write().await;
        actions.push_back(action);
        
        Ok(action_id)
    }
    
    /// Apply governance policies to content
    pub async fn apply_policies_to_content(
        &self,
        content: &Content,
    ) -> Result<Vec<PolicyAction>, GovernanceError> {
        let mut actions = Vec::new();
        let policies = self.policies.read().await;
        
        for policy in policies.values() {
            if policy.status != PolicyStatus::Active {
                continue;
            }
            
            // Check if content is in policy scope
            if !self.is_content_in_geographic_scope(content, &policy.geographic_scope) {
                continue;
            }
            
            // Apply policy rules
            for rule in &policy.rules {
                if self.evaluate_rule_condition(content, &rule.condition).await? {
                    actions.push(PolicyAction {
                        policy_id: policy.id.clone(),
                        rule_id: rule.id.clone(),
                        action: rule.action.clone(),
                        priority: rule.priority,
                    });
                }
            }
        }
        
        // Sort by priority
        actions.sort_by(|a, b| b.priority.cmp(&a.priority));
        
        Ok(actions)
    }
    
    /// Get governance statistics
    pub async fn get_governance_stats(&self) -> GovernanceStats {
        let communities = self.communities.read().await;
        let policies = self.policies.read().await;
        let actions = self.moderation_actions.read().await;
        
        GovernanceStats {
            total_communities: communities.len(),
            total_policies: policies.len(),
            active_policies: policies.values().filter(|p| p.status == PolicyStatus::Active).count(),
            total_moderation_actions: actions.len(),
            pending_moderation_actions: actions.iter().filter(|a| a.status == ModerationStatus::Pending).count(),
        }
    }
    
    /// Start policy enforcement
    async fn start_policy_enforcement(&self) -> Result<(), GovernanceError> {
        // This would start background policy enforcement
        Ok(())
    }
    
    /// Start moderation processing
    async fn start_moderation_processing(&self) -> Result<(), GovernanceError> {
        // This would start background moderation processing
        Ok(())
    }
    
    /// Start community management
    async fn start_community_management(&self) -> Result<(), GovernanceError> {
        // This would start background community management
        Ok(())
    }
    
    /// Generate community ID from geographic region
    fn generate_community_id(&self, region: &GeographicRegion) -> String {
        let (min_lat, min_lon, max_lat, max_lon) = region.bounds;
        format!("{:.6}_{:.6}_{:.6}_{:.6}", min_lat, min_lon, max_lat, max_lon)
    }
    
    /// Check if node is in geographic scope
    async fn is_node_in_geographic_scope(
        &self,
        node_id: NodeId,
        scope: &GeographicRegion,
    ) -> Result<bool, GovernanceError> {
        // This would check if the node is in the geographic scope
        // For now, return true
        Ok(true)
    }
    
    /// Find community for content
    async fn find_community_for_content(&self, content_id: &ContentId) -> Result<String, GovernanceError> {
        // This would find the appropriate community for the content
        // For now, return a default community
        Ok("default_community".to_string())
    }
    
    /// Check moderation permission
    async fn has_moderation_permission(&self, community_id: &str, moderator: &NodeId) -> Result<bool, GovernanceError> {
        let communities = self.communities.read().await;
        
        if let Some(community) = communities.get(community_id) {
            Ok(community.moderators.contains(moderator))
        } else {
            Ok(false)
        }
    }
    
    /// Check if content is in geographic scope
    fn is_content_in_geographic_scope(&self, content: &Content, scope: &GeographicRegion) -> bool {
        scope.contains(&content.location)
    }
    
    /// Evaluate rule condition
    async fn evaluate_rule_condition(
        &self,
        content: &Content,
        condition: &RuleCondition,
    ) -> Result<bool, GovernanceError> {
        match condition {
            RuleCondition::ContainsKeywords(keywords) => {
                let content_text = String::from_utf8_lossy(&content.data);
                Ok(keywords.iter().any(|keyword| content_text.contains(keyword)))
            }
            RuleCondition::FromAuthor(author) => {
                Ok(content.author == author.to_string())
            }
            RuleCondition::InGeographicRegion(region) => {
                Ok(region.contains(&content.location))
            }
            RuleCondition::HasContentType(content_type) => {
                Ok(content.content_type == *content_type)
            }
            RuleCondition::ViolatesCommunityStandards => {
                // This would implement community standards checking
                Ok(false)
            }
            RuleCondition::EmergencyCondition => {
                // This would implement emergency condition checking
                Ok(false)
            }
        }
    }
}

/// Policy vote
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PolicyVote {
    /// Approve the policy
    Approve,
    /// Reject the policy
    Reject,
}

/// Policy action
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyAction {
    /// Policy ID
    pub policy_id: String,
    /// Rule ID
    pub rule_id: String,
    /// Action to take
    pub action: RuleAction,
    /// Priority
    pub priority: u8,
}

/// Governance statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernanceStats {
    /// Total number of communities
    pub total_communities: usize,
    /// Total number of policies
    pub total_policies: usize,
    /// Number of active policies
    pub active_policies: usize,
    /// Total moderation actions
    pub total_moderation_actions: usize,
    /// Pending moderation actions
    pub pending_moderation_actions: usize,
}

/// Governance errors
#[derive(Error, Debug)]
pub enum GovernanceError {
    #[error("Community not found: {0}")]
    CommunityNotFound(String),
    
    #[error("Policy not found: {0}")]
    PolicyNotFound(String),
    
    #[error("Voter not in geographic scope")]
    VoterNotInScope,
    
    #[error("Insufficient permissions")]
    InsufficientPermissions,
    
    #[error("Invalid policy: {0}")]
    InvalidPolicy(String),
    
    #[error("Moderation failed: {0}")]
    ModerationFailed(String),
    
    #[error("Geographic error: {0}")]
    GeographicError(#[from] GeographicError),
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_governance_config_default() {
        let config = GovernanceConfig::default();
        assert_eq!(config.min_community_size, 10);
        assert_eq!(config.moderation_quorum, 3);
    }
    
    #[tokio::test]
    async fn test_governance_manager_creation() {
        let config = GovernanceConfig::default();
        let governance = GovernanceManager::new(config);
        
        let stats = governance.get_governance_stats().await;
        assert_eq!(stats.total_communities, 0);
        assert_eq!(stats.total_policies, 0);
    }
    
    #[tokio::test]
    async fn test_create_community() {
        let config = GovernanceConfig::default();
        let governance = GovernanceManager::new(config);
        
        let region = GeographicRegion {
            bounds: (37.7749, -122.4194, 37.7849, -122.4094),
            activity_level: 0.5,
            content_count: 0,
        };
        
        let creator = NodeId::new();
        let community_id = governance.create_community(region, creator).await.unwrap();
        
        assert!(!community_id.is_empty());
        
        let stats = governance.get_governance_stats().await;
        assert_eq!(stats.total_communities, 1);
    }
}