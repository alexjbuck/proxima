//! Geographic routing protocol for Proxima
//!
//! This module implements the geographic routing protocol that uses distance-vector
//! routing with geographic weights to route content and messages through the network.

use std::collections::{HashMap, HashSet, VecDeque, BinaryHeap};
use std::sync::Arc;
use std::time::{Duration, Instant};
use std::cmp::Ordering;
use serde::{Deserialize, Serialize};
use tokio::sync::{RwLock, mpsc};
use dashmap::DashMap;
use thiserror::Error;

use crate::geo::*;
use crate::content::*;
use crate::network::*;

/// A routing table entry for geographic routing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingEntry {
    /// Destination geographic address
    pub destination: GeographicAddress,
    /// Next hop node ID
    pub next_hop: NodeId,
    /// Geographic distance to destination
    pub geographic_distance: f64,
    /// Social affinity score
    pub social_affinity: f64,
    /// Staleness factor
    pub staleness_factor: f64,
    /// Combined routing cost
    pub route_cost: f64,
    /// Last update timestamp
    pub last_updated: chrono::DateTime<chrono::Utc>,
    /// Number of hops to destination
    pub hop_count: u32,
}

/// A routing table organized by geographic sectors
#[derive(Debug, Clone)]
pub struct GeographicRoutingTable {
    /// Routing entries organized by geographic sector
    entries: HashMap<GeographicSector, Vec<RoutingEntry>>,
    /// Entries organized by destination geohash
    by_destination: HashMap<String, RoutingEntry>,
    /// Maximum number of entries per sector
    max_entries_per_sector: usize,
    /// Last cleanup time
    last_cleanup: Instant,
}

/// Geographic routing table manager
pub struct RoutingTable {
    /// Geographic routing table
    routing_table: Arc<RwLock<GeographicRoutingTable>>,
    /// Node information cache
    node_cache: Arc<DashMap<NodeId, NodeInfo>>,
    /// Route update channel
    route_update_tx: mpsc::UnboundedSender<RouteUpdate>,
    /// Route update receiver
    route_update_rx: Arc<RwLock<mpsc::UnboundedReceiver<RouteUpdate>>>,
    /// Configuration
    config: RoutingConfig,
}

/// Node information for routing decisions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInfo {
    /// Node ID
    pub id: NodeId,
    /// Node location
    pub location: GeographicLocation,
    /// Node capabilities
    pub capabilities: NodeCapabilities,
    /// Last seen timestamp
    pub last_seen: chrono::DateTime<chrono::Utc>,
    /// Social connections to this node
    pub social_connections: HashSet<NodeId>,
    /// Mobility profile
    pub mobility_profile: Option<MobilityProfile>,
}

/// Node capabilities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeCapabilities {
    /// Can store content
    pub can_store: bool,
    /// Can route messages
    pub can_route: bool,
    /// Can bridge geographic regions
    pub can_bridge: bool,
    /// Storage capacity in bytes
    pub storage_capacity: u64,
    /// Bandwidth capacity in bytes per second
    pub bandwidth_capacity: u64,
}

/// Mobility profile for bridge nodes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MobilityProfile {
    /// Home region
    pub home_region: GeographicAddress,
    /// Work region
    pub work_region: Option<GeographicAddress>,
    /// Frequency map of visited regions
    pub frequency_map: HashMap<String, f64>, // geohash -> frequency
    /// Typical routes
    pub typical_routes: Vec<RouteSignature>,
    /// Bridge capacity
    pub bridge_capacity: u64,
}

/// Route signature for mobility tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteSignature {
    /// Start location
    pub start: GeographicAddress,
    /// End location
    pub end: GeographicAddress,
    /// Typical travel time
    pub travel_time: Duration,
    /// Frequency of this route
    pub frequency: f64,
}

/// Route update message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RouteUpdate {
    /// New route discovered
    NewRoute(RoutingEntry),
    /// Route updated
    UpdateRoute(RoutingEntry),
    /// Route removed
    RemoveRoute(GeographicAddress),
    /// Node information updated
    UpdateNode(NodeInfo),
    /// Node removed
    RemoveNode(NodeId),
}

/// Routing configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingConfig {
    /// Maximum hop distance in meters
    pub max_hop_distance: f64,
    /// Maximum routing table size
    pub max_table_size: usize,
    /// Route update interval
    pub update_interval: Duration,
    /// Route expiration time
    pub route_ttl: Duration,
    /// Maximum number of entries per sector
    pub max_entries_per_sector: usize,
    /// Geographic decay factor
    pub geographic_decay_factor: f64,
    /// Social affinity weight
    pub social_affinity_weight: f64,
    /// Staleness weight
    pub staleness_weight: f64,
}

impl Default for RoutingConfig {
    fn default() -> Self {
        Self {
            max_hop_distance: 10000.0, // 10km
            max_table_size: 10000,
            update_interval: Duration::from_secs(30),
            route_ttl: Duration::from_secs(300), // 5 minutes
            max_entries_per_sector: 100,
            geographic_decay_factor: 0.1,
            social_affinity_weight: 0.3,
            staleness_weight: 0.2,
        }
    }
}

/// Priority queue item for route selection
#[derive(Debug, Clone)]
struct RoutePriority {
    route_cost: f64,
    entry: RoutingEntry,
}

impl PartialEq for RoutePriority {
    fn eq(&self, other: &Self) -> bool {
        self.route_cost == other.route_cost
    }
}

impl Eq for RoutePriority {}

impl PartialOrd for RoutePriority {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        // Reverse ordering for min-heap (lower cost = higher priority)
        other.route_cost.partial_cmp(&self.route_cost)
    }
}

impl Ord for RoutePriority {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap_or(Ordering::Equal)
    }
}

impl RoutingTable {
    /// Create a new routing table
    pub fn new() -> Self {
        let (route_update_tx, route_update_rx) = mpsc::unbounded_channel();
        
        Self {
            routing_table: Arc::new(RwLock::new(GeographicRoutingTable {
                entries: HashMap::new(),
                by_destination: HashMap::new(),
                max_entries_per_sector: 100,
                last_cleanup: Instant::now(),
            })),
            node_cache: Arc::new(DashMap::new()),
            route_update_tx,
            route_update_rx: Arc::new(RwLock::new(route_update_rx)),
            config: RoutingConfig::default(),
        }
    }
    
    /// Start geographic routing
    pub async fn start_geographic_routing(&self) -> Result<(), RoutingError> {
        // Start route update processing
        let routing_table = self.routing_table.clone();
        let node_cache = self.node_cache.clone();
        let config = self.config.clone();
        
        tokio::spawn(async move {
            Self::route_update_loop(routing_table, node_cache, config).await;
        });
        
        // Start periodic cleanup
        let routing_table = self.routing_table.clone();
        let config = self.config.clone();
        
        tokio::spawn(async move {
            Self::cleanup_loop(routing_table, config).await;
        });
        
        Ok(())
    }
    
    /// Add a new route
    pub async fn add_route(&self, entry: RoutingEntry) -> Result<(), RoutingError> {
        let mut table = self.routing_table.write().await;
        
        // Determine geographic sector
        let sector = GeographicSector::from_relative_location(
            &GeographicLocation::new(0.0, 0.0, 0.0).unwrap(), // Reference point
            &GeographicLocation::new(
                entry.destination.coordinates()?.0,
                entry.destination.coordinates()?.1,
                0.0,
            ).unwrap(),
        );
        
        // Add to sector entries
        let sector_entries = table.entries.entry(sector).or_insert_with(Vec::new);
        
        // Check if we already have a route to this destination
        if let Some(existing_index) = sector_entries.iter().position(|e| e.destination.geohash == entry.destination.geohash) {
            // Update existing route if new one is better
            if entry.route_cost < sector_entries[existing_index].route_cost {
                sector_entries[existing_index] = entry.clone();
            }
        } else {
            // Add new route
            sector_entries.push(entry.clone());
            
            // Limit entries per sector
            if sector_entries.len() > table.max_entries_per_sector {
                sector_entries.sort_by(|a, b| a.route_cost.partial_cmp(&b.route_cost).unwrap());
                sector_entries.truncate(table.max_entries_per_sector);
            }
        }
        
        // Update by-destination index
        table.by_destination.insert(entry.destination.geohash.clone(), entry);
        
        Ok(())
    }
    
    /// Find the best route to a destination
    pub async fn find_route(&self, destination: &GeographicAddress) -> Option<RoutingEntry> {
        let table = self.routing_table.read().await;
        
        // First try direct lookup
        if let Some(entry) = table.by_destination.get(&destination.geohash) {
            return Some(entry.clone());
        }
        
        // Find best route by sector
        let sector = GeographicSector::from_relative_location(
            &GeographicLocation::new(0.0, 0.0, 0.0).unwrap(),
            &GeographicLocation::new(
                destination.coordinates().unwrap().0,
                destination.coordinates().unwrap().1,
                0.0,
            ).unwrap(),
        );
        
        if let Some(sector_entries) = table.entries.get(&sector) {
            // Return the route with lowest cost
            sector_entries.iter()
                .min_by(|a, b| a.route_cost.partial_cmp(&b.route_cost).unwrap())
                .cloned()
        } else {
            None
        }
    }
    
    /// Route content to a geographic region
    pub async fn route_content(&self, content_id: &ContentId, origin_location: &GeographicLocation) -> Result<(), RoutingError> {
        // This is a simplified implementation
        // In a real system, this would:
        // 1. Determine the target geographic region based on content metadata
        // 2. Find the best routes to nodes in that region
        // 3. Send content to those nodes
        
        tracing::info!(
            "Routing content {} from location {}",
            content_id,
            origin_location
        );
        
        Ok(())
    }
    
    /// Update node information
    pub async fn update_node(&self, node_info: NodeInfo) -> Result<(), RoutingError> {
        self.node_cache.insert(node_info.id.clone(), node_info);
        Ok(())
    }
    
    /// Get node information
    pub async fn get_node(&self, node_id: &NodeId) -> Option<NodeInfo> {
        self.node_cache.get(node_id).map(|entry| entry.clone())
    }
    
    /// Calculate route cost using geographic routing metrics
    pub fn calculate_route_cost(
        &self,
        geographic_distance: f64,
        social_affinity: f64,
        staleness_factor: f64,
    ) -> f64 {
        geographic_distance * (1.0 - social_affinity * self.config.social_affinity_weight) * staleness_factor
    }
    
    /// Background route update processing loop
    async fn route_update_loop(
        routing_table: Arc<RwLock<GeographicRoutingTable>>,
        node_cache: Arc<DashMap<NodeId, NodeInfo>>,
        config: RoutingConfig,
    ) {
        let mut interval = tokio::time::interval(config.update_interval);
        
        loop {
            interval.tick().await;
            
            // Process route updates
            // This would typically process updates from the route_update_rx channel
            // For now, we'll just log that we're running
            tracing::debug!("Route update loop running");
        }
    }
    
    /// Background cleanup loop
    async fn cleanup_loop(
        routing_table: Arc<RwLock<GeographicRoutingTable>>,
        config: RoutingConfig,
    ) {
        let mut interval = tokio::time::interval(Duration::from_secs(60)); // 1 minute
        
        loop {
            interval.tick().await;
            
            let mut table = routing_table.write().await;
            let now = chrono::Utc::now();
            
            // Remove expired routes
            for sector_entries in table.entries.values_mut() {
                sector_entries.retain(|entry| {
                    (now - entry.last_updated).to_std().unwrap_or(Duration::ZERO) < config.route_ttl
                });
            }
            
            // Clean up by-destination index
            table.by_destination.retain(|_, entry| {
                (now - entry.last_updated).to_std().unwrap_or(Duration::ZERO) < config.route_ttl
            });
            
            table.last_cleanup = Instant::now();
        }
    }
}

/// Geographic routing algorithm implementation
pub struct GeographicRouter {
    /// Routing table
    routing_table: Arc<RoutingTable>,
    /// Node location
    node_location: GeographicLocation,
    /// Node ID
    node_id: NodeId,
}

impl GeographicRouter {
    /// Create a new geographic router
    pub fn new(
        routing_table: Arc<RoutingTable>,
        node_location: GeographicLocation,
        node_id: NodeId,
    ) -> Self {
        Self {
            routing_table,
            node_location,
            node_id,
        }
    }
    
    /// Route a message to a geographic destination
    pub async fn route_message(
        &self,
        destination: &GeographicAddress,
        message: &[u8],
    ) -> Result<Vec<NodeId>, RoutingError> {
        // Find the best route to the destination
        if let Some(route) = self.routing_table.find_route(destination).await {
            // For now, return the next hop
            // In a real implementation, this would send the message
            Ok(vec![route.next_hop])
        } else {
            // No route found, try to find a bridge node
            self.find_bridge_route(destination).await
        }
    }
    
    /// Find a bridge route to a distant destination
    async fn find_bridge_route(&self, destination: &GeographicAddress) -> Result<Vec<NodeId>, RoutingError> {
        // Look for nodes with mobility profiles that might reach the destination
        let mut bridge_candidates = Vec::new();
        
        // This is a simplified implementation
        // In a real system, this would:
        // 1. Query nodes with mobility profiles
        // 2. Find nodes that frequently visit the destination region
        // 3. Select the best bridge node
        
        Ok(bridge_candidates)
    }
    
    /// Select the best next hop for geographic routing
    pub async fn select_next_hop(
        &self,
        destination: &GeographicAddress,
        available_neighbors: &[NodeId],
    ) -> Option<NodeId> {
        let mut best_neighbor = None;
        let mut best_score = f64::INFINITY;
        
        for neighbor_id in available_neighbors {
            if let Some(neighbor_info) = self.routing_table.get_node(neighbor_id).await {
                // Calculate routing score
                let geographic_distance = self.node_location.distance_to(&neighbor_info.location);
                let social_affinity = if neighbor_info.social_connections.contains(&self.node_id) {
                    1.0
                } else {
                    0.0
                };
                let staleness_factor = self.calculate_staleness_factor(&neighbor_info.last_seen);
                
                let route_cost = self.routing_table.calculate_route_cost(
                    geographic_distance,
                    social_affinity,
                    staleness_factor,
                );
                
                if route_cost < best_score {
                    best_score = route_cost;
                    best_neighbor = Some(neighbor_id.clone());
                }
            }
        }
        
        best_neighbor
    }
    
    /// Calculate staleness factor for a timestamp
    fn calculate_staleness_factor(&self, last_seen: &chrono::DateTime<chrono::Utc>) -> f64 {
        let age_seconds = (chrono::Utc::now() - *last_seen).num_seconds() as f64;
        let decay_constant = 300.0; // 5 minutes
        (-age_seconds / decay_constant).exp()
    }
}

/// Routing errors
#[derive(Error, Debug)]
pub enum RoutingError {
    #[error("No route found to destination")]
    NoRouteFound,
    
    #[error("Route calculation failed: {0}")]
    RouteCalculationFailed(String),
    
    #[error("Node not found: {0}")]
    NodeNotFound(NodeId),
    
    #[error("Geographic error: {0}")]
    GeographicError(#[from] GeographicError),
    
    #[error("Network error: {0}")]
    NetworkError(#[from] NetworkError),
    
    #[error("Invalid route entry")]
    InvalidRouteEntry,
    
    #[error("Routing table full")]
    RoutingTableFull,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_routing_table_operations() {
        let routing_table = RoutingTable::new();
        
        let location = GeographicLocation::new(37.7749, -122.4194, 10.0).unwrap();
        let address = GeographicAddress::new(37.7749, -122.4194, GeographicLayer::Neighborhood, 0.9).unwrap();
        
        let entry = RoutingEntry {
            destination: address.clone(),
            next_hop: NodeId::new(),
            geographic_distance: 1000.0,
            social_affinity: 0.8,
            staleness_factor: 0.9,
            route_cost: 100.0,
            last_updated: chrono::Utc::now(),
            hop_count: 1,
        };
        
        routing_table.add_route(entry).await.unwrap();
        
        let found_route = routing_table.find_route(&address).await;
        assert!(found_route.is_some());
    }
    
    #[test]
    fn test_route_cost_calculation() {
        let routing_table = RoutingTable::new();
        
        let cost = routing_table.calculate_route_cost(1000.0, 0.8, 0.9);
        assert!(cost > 0.0);
    }
}