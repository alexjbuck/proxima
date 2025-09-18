//! Proxima - A decentralized geographic social network
//! 
//! This library provides the core geographic infrastructure for location-based
//! networking, content distribution, and routing.

pub mod geo;
pub mod cache;
pub mod utils;

// Re-export main types for convenience
pub use geo::{
    GeographicLayer, GeographicAddress, GeographicLocation, GeographicSector,
    GeographicRoutingTable, GeographicRoute, GeographicContentRelevance,
    GeographicContentGravity, GeographicError,
};

pub use cache::{
    HTMIndex, QuadTree, RTree, GeographicBloomFilter, SpatialContentIndex,
    ContentMetadata,
};

pub use utils::{
    LocationPrecisionAdjuster, AnchorPointDetector, MobilityAnalyzer,
    BoundaryDetector, VoronoiDiagram,
};

/// Geographic routing algorithm implementation
pub mod routing {
    use std::collections::{HashMap, HashSet, VecDeque, BinaryHeap};
    use std::cmp::Ordering;
    use std::sync::{Arc, RwLock};
    use std::time::{SystemTime, UNIX_EPOCH};

    use ahash::{AHashMap, AHashSet};
    use rayon::prelude::*;

    use crate::geo::{
        GeographicLocation, GeographicLayer, GeographicSector, GeographicRoute,
        GeographicRoutingTable, GeographicError,
    };

    /// Geographic routing algorithm with distance-vector routing
    #[derive(Debug, Clone)]
    pub struct GeographicRouter {
        /// Local routing table
        routing_table: GeographicRoutingTable,
        /// Neighbor nodes
        neighbors: AHashMap<String, NeighborNode>,
        /// Route update queue
        update_queue: VecDeque<RouteUpdate>,
        /// Last update timestamp
        last_update: u64,
        /// Routing parameters
        params: RoutingParameters,
    }

    #[derive(Debug, Clone)]
    pub struct NeighborNode {
        /// Node ID
        pub node_id: String,
        /// Location
        pub location: GeographicLocation,
        /// Sector
        pub sector: GeographicSector,
        /// Connection quality (0-1)
        pub quality: f64,
        /// Last seen timestamp
        pub last_seen: u64,
        /// Routing cost to this neighbor
        pub cost: f64,
    }

    #[derive(Debug, Clone)]
    pub struct RouteUpdate {
        /// Source node
        pub source: String,
        /// Destination sector
        pub destination: String,
        /// Route information
        pub route: GeographicRoute,
        /// Timestamp
        pub timestamp: u64,
    }

    #[derive(Debug, Clone)]
    pub struct RoutingParameters {
        /// Maximum hop count
        pub max_hops: usize,
        /// Route timeout (seconds)
        pub route_timeout: u64,
        /// Update interval (seconds)
        pub update_interval: u64,
        /// Geographic weight factor
        pub geo_weight: f64,
        /// Density weight factor
        pub density_weight: f64,
        /// Quality weight factor
        pub quality_weight: f64,
    }

    impl Default for RoutingParameters {
        fn default() -> Self {
            RoutingParameters {
                max_hops: 10,
                route_timeout: 300, // 5 minutes
                update_interval: 30, // 30 seconds
                geo_weight: 0.4,
                density_weight: 0.3,
                quality_weight: 0.3,
            }
        }
    }

    impl GeographicRouter {
        /// Create a new geographic router
        pub fn new(local_sector: GeographicSector, params: RoutingParameters) -> Self {
            GeographicRouter {
                routing_table: GeographicRoutingTable::new(local_sector),
                neighbors: AHashMap::new(),
                update_queue: VecDeque::new(),
                last_update: Self::current_timestamp(),
                params,
            }
        }

        /// Add a neighbor node
        pub fn add_neighbor(&mut self, neighbor: NeighborNode) {
            let node_id = neighbor.node_id.clone();
            self.neighbors.insert(node_id, neighbor);
        }

        /// Remove a neighbor node
        pub fn remove_neighbor(&mut self, node_id: &str) {
            self.neighbors.remove(node_id);
        }

        /// Update neighbor information
        pub fn update_neighbor(&mut self, node_id: &str, location: GeographicLocation, quality: f64) {
            if let Some(neighbor) = self.neighbors.get_mut(node_id) {
                neighbor.location = location;
                neighbor.quality = quality;
                neighbor.last_seen = Self::current_timestamp();
                neighbor.cost = self.calculate_neighbor_cost(neighbor);
            }
        }

        /// Calculate routing cost to a neighbor
        fn calculate_neighbor_cost(&self, neighbor: &NeighborNode) -> f64 {
            let distance = self.routing_table.local_sector.center.distance_to(&neighbor.location);
            let density_factor = 1.0 / (1.0 + neighbor.sector.content_density);
            let quality_factor = 1.0 / neighbor.quality;
            
            self.params.geo_weight * distance / 1000.0 +
            self.params.density_weight * density_factor +
            self.params.quality_weight * quality_factor
        }

        /// Find the best route to a destination
        pub fn find_route(&self, destination: GeographicLocation, layer: GeographicLayer) -> Option<GeographicRoute> {
            // First check if destination is in local sector
            if self.routing_table.local_sector.contains(&destination) {
                return Some(GeographicRoute {
                    destination: self.routing_table.local_sector.id.clone(),
                    next_hop: self.routing_table.local_sector.id.clone(),
                    distance: self.routing_table.local_sector.center.distance_to(&destination),
                    cost: 0.0,
                    last_updated: Self::current_timestamp(),
                });
            }

            // Use Dijkstra's algorithm to find shortest path
            self.dijkstra_route(destination, layer)
        }

        /// Dijkstra's algorithm for geographic routing
        fn dijkstra_route(&self, destination: GeographicLocation, layer: GeographicLayer) -> Option<GeographicRoute> {
            let mut distances: AHashMap<String, f64> = AHashMap::new();
            let mut previous: AHashMap<String, String> = AHashMap::new();
            let mut visited: AHashSet<String> = AHashSet::new();
            let mut queue: BinaryHeap<RouteNode> = BinaryHeap::new();

            // Initialize distances
            for (node_id, neighbor) in &self.neighbors {
                distances.insert(node_id.clone(), f64::INFINITY);
            }
            distances.insert(self.routing_table.local_sector.id.clone(), 0.0);

            // Start from local sector
            queue.push(RouteNode {
                id: self.routing_table.local_sector.id.clone(),
                cost: 0.0,
            });

            while let Some(current) = queue.pop() {
                if visited.contains(&current.id) {
                    continue;
                }
                visited.insert(current.id.clone());

                // Check if we've reached the destination
                if current.id == self.find_sector_for_location(destination, layer) {
                    return self.reconstruct_route(&previous, &current.id, destination);
                }

                // Explore neighbors
                for (neighbor_id, neighbor) in &self.neighbors {
                    if visited.contains(neighbor_id) {
                        continue;
                    }

                    let edge_cost = self.calculate_edge_cost(&current.id, neighbor_id);
                    let new_cost = distances.get(&current.id).unwrap_or(&f64::INFINITY) + edge_cost;

                    if new_cost < *distances.get(neighbor_id).unwrap_or(&f64::INFINITY) {
                        distances.insert(neighbor_id.clone(), new_cost);
                        previous.insert(neighbor_id.clone(), current.id.clone());
                        queue.push(RouteNode {
                            id: neighbor_id.clone(),
                            cost: new_cost,
                        });
                    }
                }
            }

            None
        }

        /// Calculate edge cost between two nodes
        fn calculate_edge_cost(&self, from: &str, to: &str) -> f64 {
            if let Some(neighbor) = self.neighbors.get(to) {
                neighbor.cost
            } else {
                f64::INFINITY
            }
        }

        /// Find sector for a location at a given layer
        fn find_sector_for_location(&self, location: GeographicLocation, layer: GeographicLayer) -> String {
            // Simple implementation - could be enhanced with actual sector lookup
            format!("sector_{}_{}_{}", 
                (location.lat * 100.0) as i32, 
                (location.lon * 100.0) as i32,
                layer.geohash_precision()
            )
        }

        /// Reconstruct route from previous map
        fn reconstruct_route(
            &self,
            previous: &AHashMap<String, String>,
            destination: &str,
            final_destination: GeographicLocation,
        ) -> Option<GeographicRoute> {
            let mut path = Vec::new();
            let mut current = destination.to_string();

            while let Some(prev) = previous.get(&current) {
                path.push(current);
                current = prev.clone();
            }
            path.push(current);

            if path.len() < 2 {
                return None;
            }

            let next_hop = path[path.len() - 2].clone();
            let distance = self.calculate_total_distance(&path, final_destination);
            let cost = self.calculate_total_cost(&path);

            Some(GeographicRoute {
                destination: destination.to_string(),
                next_hop,
                distance,
                cost,
                last_updated: Self::current_timestamp(),
            })
        }

        /// Calculate total distance for a path
        fn calculate_total_distance(&self, path: &[String], final_destination: GeographicLocation) -> f64 {
            let mut total_distance = 0.0;
            let mut current_location = self.routing_table.local_sector.center;

            for sector_id in path.iter().rev() {
                if let Some(neighbor) = self.neighbors.get(sector_id) {
                    total_distance += current_location.distance_to(&neighbor.location);
                    current_location = neighbor.location;
                }
            }

            total_distance += current_location.distance_to(&final_destination);
            total_distance
        }

        /// Calculate total cost for a path
        fn calculate_total_cost(&self, path: &[String]) -> f64 {
            let mut total_cost = 0.0;

            for sector_id in path.iter().rev() {
                if let Some(neighbor) = self.neighbors.get(sector_id) {
                    total_cost += neighbor.cost;
                }
            }

            total_cost
        }

        /// Process route updates from neighbors
        pub fn process_route_updates(&mut self) {
            let current_time = Self::current_timestamp();

            while let Some(update) = self.update_queue.pop_front() {
                // Check if update is still valid
                if current_time - update.timestamp > self.params.route_timeout {
                    continue;
                }

                // Update routing table
                self.update_routing_table(update);
            }

            self.last_update = current_time;
        }

        /// Update routing table with new route information
        fn update_routing_table(&mut self, update: RouteUpdate) {
            let layer = self.determine_layer_for_route(&update.route);
            
            // Check if this route is better than existing one
            if let Some(existing_route) = self.routing_table.get_route(layer, &update.route.destination) {
                if update.route.cost < existing_route.cost {
                    self.routing_table.update_route(layer, update.route);
                }
            } else {
                self.routing_table.update_route(layer, update.route);
            }
        }

        /// Determine layer for a route
        fn determine_layer_for_route(&self, route: &GeographicRoute) -> GeographicLayer {
            // Simple implementation - could be enhanced with actual layer determination
            if route.distance < 1000.0 {
                GeographicLayer::Neighborhood
            } else if route.distance < 10000.0 {
                GeographicLayer::District
            } else if route.distance < 100000.0 {
                GeographicLayer::City
            } else if route.distance < 1000000.0 {
                GeographicLayer::Region
            } else {
                GeographicLayer::Global
            }
        }

        /// Broadcast route updates to neighbors
        pub fn broadcast_routes(&mut self) {
            let current_time = Self::current_timestamp();
            
            // Only broadcast if enough time has passed
            if current_time - self.last_update < self.params.update_interval {
                return;
            }

            for (neighbor_id, _) in &self.neighbors {
                self.send_route_update(neighbor_id, current_time);
            }
        }

        /// Send route update to a neighbor
        fn send_route_update(&mut self, neighbor_id: &str, timestamp: u64) {
            // This would typically send over the network
            // For now, we'll just log the update
            log::debug!("Sending route update to neighbor: {}", neighbor_id);
        }

        /// Get current timestamp
        fn current_timestamp() -> u64 {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs()
        }

        /// Get routing table
        pub fn get_routing_table(&self) -> &GeographicRoutingTable {
            &self.routing_table
        }

        /// Get neighbors
        pub fn get_neighbors(&self) -> &AHashMap<String, NeighborNode> {
            &self.neighbors
        }

        /// Clean up old routes
        pub fn cleanup_old_routes(&mut self) {
            let current_time = Self::current_timestamp();
            
            // Remove old neighbors
            self.neighbors.retain(|_, neighbor| {
                current_time - neighbor.last_seen < self.params.route_timeout
            });

            // Remove old routes from routing table
            for (_, routes) in self.routing_table.routes.iter_mut() {
                routes.retain(|_, route| {
                    current_time - route.last_updated < self.params.route_timeout
                });
            }
        }
    }

    /// Route node for Dijkstra's algorithm
    #[derive(Debug, Clone)]
    struct RouteNode {
        id: String,
        cost: f64,
    }

    impl PartialEq for RouteNode {
        fn eq(&self, other: &Self) -> bool {
            self.cost == other.cost
        }
    }

    impl Eq for RouteNode {}

    impl PartialOrd for RouteNode {
        fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
            // Reverse ordering for min-heap
            other.cost.partial_cmp(&self.cost)
        }
    }

    impl Ord for RouteNode {
        fn cmp(&self, other: &Self) -> Ordering {
            self.partial_cmp(other).unwrap()
        }
    }

    /// Geographic routing with content gravity
    #[derive(Debug, Clone)]
    pub struct ContentAwareRouter {
        /// Base geographic router
        router: GeographicRouter,
        /// Content gravity map
        content_gravity: AHashMap<String, f64>,
        /// Content relevance cache
        relevance_cache: AHashMap<String, f64>,
    }

    impl ContentAwareRouter {
        /// Create a new content-aware router
        pub fn new(local_sector: GeographicSector, params: RoutingParameters) -> Self {
            ContentAwareRouter {
                router: GeographicRouter::new(local_sector, params),
                content_gravity: AHashMap::new(),
                relevance_cache: AHashMap::new(),
            }
        }

        /// Find route considering content gravity
        pub fn find_content_route(
            &self,
            destination: GeographicLocation,
            content_type: &str,
            layer: GeographicLayer,
        ) -> Option<GeographicRoute> {
            // Get base route
            let mut route = self.router.find_route(destination, layer)?;

            // Adjust cost based on content gravity
            if let Some(gravity) = self.content_gravity.get(content_type) {
                route.cost *= (1.0 - gravity * 0.2); // Reduce cost by up to 20% for high gravity
            }

            Some(route)
        }

        /// Update content gravity
        pub fn update_content_gravity(&mut self, content_type: String, gravity: f64) {
            self.content_gravity.insert(content_type, gravity);
        }

        /// Get content gravity
        pub fn get_content_gravity(&self, content_type: &str) -> f64 {
            self.content_gravity.get(content_type).copied().unwrap_or(0.0)
        }

        /// Update content relevance
        pub fn update_content_relevance(&mut self, content_id: String, relevance: f64) {
            self.relevance_cache.insert(content_id, relevance);
        }

        /// Get content relevance
        pub fn get_content_relevance(&self, content_id: &str) -> f64 {
            self.relevance_cache.get(content_id).copied().unwrap_or(0.0)
        }

        /// Get base router
        pub fn get_router(&self) -> &GeographicRouter {
            &self.router
        }

        /// Get mutable base router
        pub fn get_router_mut(&mut self) -> &mut GeographicRouter {
            &mut self.router
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn test_geographic_router_creation() {
            let sector = GeographicSector::new(
                "test_sector".to_string(),
                GeographicLocation::new(40.7128, -74.0060).unwrap(),
                1000.0,
                GeographicLayer::Neighborhood,
            );
            
            let params = RoutingParameters::default();
            let router = GeographicRouter::new(sector, params);
            
            assert_eq!(router.get_routing_table().local_sector.id, "test_sector");
        }

        #[test]
        fn test_neighbor_management() {
            let sector = GeographicSector::new(
                "test_sector".to_string(),
                GeographicLocation::new(40.7128, -74.0060).unwrap(),
                1000.0,
                GeographicLayer::Neighborhood,
            );
            
            let params = RoutingParameters::default();
            let mut router = GeographicRouter::new(sector, params);
            
            let neighbor = NeighborNode {
                node_id: "neighbor1".to_string(),
                location: GeographicLocation::new(40.7130, -74.0058).unwrap(),
                sector: GeographicSector::new(
                    "neighbor_sector".to_string(),
                    GeographicLocation::new(40.7130, -74.0058).unwrap(),
                    1000.0,
                    GeographicLayer::Neighborhood,
                ),
                quality: 0.8,
                last_seen: 1234567890,
                cost: 1.0,
            };
            
            router.add_neighbor(neighbor);
            assert_eq!(router.get_neighbors().len(), 1);
            
            router.remove_neighbor("neighbor1");
            assert_eq!(router.get_neighbors().len(), 0);
        }

        #[test]
        fn test_content_aware_router() {
            let sector = GeographicSector::new(
                "test_sector".to_string(),
                GeographicLocation::new(40.7128, -74.0060).unwrap(),
                1000.0,
                GeographicLayer::Neighborhood,
            );
            
            let params = RoutingParameters::default();
            let mut router = ContentAwareRouter::new(sector, params);
            
            router.update_content_gravity("post".to_string(), 0.8);
            assert_eq!(router.get_content_gravity("post"), 0.8);
            
            router.update_content_relevance("content1".to_string(), 0.9);
            assert_eq!(router.get_content_relevance("content1"), 0.9);
        }
    }
}

/// Geographic content distribution
pub mod content {
    use std::collections::HashMap;
    use std::sync::{Arc, RwLock};

    use ahash::AHashMap;

    use crate::geo::{
        GeographicLocation, GeographicContentRelevance, GeographicContentGravity,
        GeographicError,
    };

    /// Content distribution manager
    #[derive(Debug)]
    pub struct ContentDistributor {
        /// Content cache
        content_cache: AHashMap<String, ContentItem>,
        /// Relevance calculator
        relevance_calculator: RelevanceCalculator,
        /// Gravity calculator
        gravity_calculator: GravityCalculator,
        /// Distribution parameters
        params: DistributionParameters,
    }

    #[derive(Debug, Clone)]
    pub struct ContentItem {
        /// Content ID
        pub id: String,
        /// Content type
        pub content_type: String,
        /// Origin location
        pub origin: GeographicLocation,
        /// Content data
        pub data: String,
        /// Timestamp
        pub timestamp: u64,
        /// Relevance score
        pub relevance: f64,
        /// Distribution radius
        pub distribution_radius: f64,
    }

    #[derive(Debug, Clone)]
    pub struct DistributionParameters {
        /// Default distribution radius (meters)
        pub default_radius: f64,
        /// Maximum distribution radius (meters)
        pub max_radius: f64,
        /// Relevance decay factor
        pub decay_factor: f64,
        /// Minimum relevance threshold
        pub min_relevance: f64,
        /// Content gravity influence
        pub gravity_influence: f64,
    }

    impl Default for DistributionParameters {
        fn default() -> Self {
            DistributionParameters {
                default_radius: 1000.0, // 1km
                max_radius: 10000.0,    // 10km
                decay_factor: 0.1,
                min_relevance: 0.1,
                gravity_influence: 0.3,
            }
        }
    }

    impl ContentDistributor {
        /// Create a new content distributor
        pub fn new(params: DistributionParameters) -> Self {
            ContentDistributor {
                content_cache: AHashMap::new(),
                relevance_calculator: RelevanceCalculator::new(),
                gravity_calculator: GravityCalculator::new(),
                params,
            }
        }

        /// Add content to the distributor
        pub fn add_content(&mut self, content: ContentItem) {
            let content_id = content.id.clone();
            self.content_cache.insert(content_id, content);
        }

        /// Get content by ID
        pub fn get_content(&self, content_id: &str) -> Option<&ContentItem> {
            self.content_cache.get(content_id)
        }

        /// Calculate content relevance for a location
        pub fn calculate_relevance(
            &self,
            content_id: &str,
            location: GeographicLocation,
        ) -> Result<GeographicContentRelevance, GeographicError> {
            let content = self.content_cache.get(content_id)
                .ok_or_else(|| GeographicError::SpatialIndexingError("Content not found".to_string()))?;

            let relevance = self.relevance_calculator.calculate(
                content_id.to_string(),
                content.origin,
                location,
                content.relevance,
                self.params.decay_factor,
            );

            Ok(relevance)
        }

        /// Get content within a radius of a location
        pub fn get_content_in_radius(
            &self,
            location: GeographicLocation,
            radius: f64,
        ) -> Vec<&ContentItem> {
            self.content_cache
                .values()
                .filter(|content| {
                    let distance = content.origin.distance_to(&location);
                    distance <= radius
                })
                .collect()
        }

        /// Get relevant content for a location
        pub fn get_relevant_content(
            &self,
            location: GeographicLocation,
            limit: usize,
        ) -> Vec<&ContentItem> {
            let mut relevant_content: Vec<_> = self.content_cache
                .values()
                .filter_map(|content| {
                    let relevance = self.relevance_calculator.calculate(
                        content.id.clone(),
                        content.origin,
                        location,
                        content.relevance,
                        self.params.decay_factor,
                    );
                    
                    if relevance.relevance >= self.params.min_relevance {
                        Some((content, relevance.relevance))
                    } else {
                        None
                    }
                })
                .collect();

            // Sort by relevance
            relevant_content.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
            
            // Return top results
            relevant_content
                .into_iter()
                .take(limit)
                .map(|(content, _)| content)
                .collect()
        }

        /// Update content gravity
        pub fn update_content_gravity(&mut self, content_type: &str) {
            let content_locations: Vec<GeographicLocation> = self.content_cache
                .values()
                .filter(|content| content.content_type == content_type)
                .map(|content| content.origin)
                .collect();

            if !content_locations.is_empty() {
                let gravity = self.gravity_calculator.calculate(
                    content_type.to_string(),
                    &content_locations,
                    self.params.max_radius,
                );
                
                self.gravity_calculator.update_gravity(content_type.to_string(), gravity);
            }
        }

        /// Get content gravity for a type
        pub fn get_content_gravity(&self, content_type: &str) -> Option<&GeographicContentGravity> {
            self.gravity_calculator.get_gravity(content_type)
        }

        /// Clean up old content
        pub fn cleanup_old_content(&mut self, current_time: u64, max_age: u64) {
            self.content_cache.retain(|_, content| {
                current_time - content.timestamp < max_age
            });
        }
    }

    /// Relevance calculator
    #[derive(Debug)]
    pub struct RelevanceCalculator {
        /// Cache for calculated relevances
        relevance_cache: AHashMap<String, GeographicContentRelevance>,
    }

    impl RelevanceCalculator {
        pub fn new() -> Self {
            RelevanceCalculator {
                relevance_cache: AHashMap::new(),
            }
        }

        pub fn calculate(
            &self,
            content_id: String,
            origin: GeographicLocation,
            current: GeographicLocation,
            base_relevance: f64,
            decay_factor: f64,
        ) -> GeographicContentRelevance {
            GeographicContentRelevance::calculate(
                content_id,
                origin,
                current,
                base_relevance,
                decay_factor,
            )
        }
    }

    /// Gravity calculator
    #[derive(Debug)]
    pub struct GravityCalculator {
        /// Gravity cache
        gravity_cache: AHashMap<String, GeographicContentGravity>,
    }

    impl GravityCalculator {
        pub fn new() -> Self {
            GravityCalculator {
                gravity_cache: AHashMap::new(),
            }
        }

        pub fn calculate(
            &self,
            content_id: String,
            content_locations: &[GeographicLocation],
            influence_radius: f64,
        ) -> GeographicContentGravity {
            GeographicContentGravity::calculate(
                content_id,
                content_locations,
                influence_radius,
            )
        }

        pub fn update_gravity(&mut self, content_type: String, gravity: GeographicContentGravity) {
            self.gravity_cache.insert(content_type, gravity);
        }

        pub fn get_gravity(&self, content_type: &str) -> Option<&GeographicContentGravity> {
            self.gravity_cache.get(content_type)
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn test_content_distributor() {
            let params = DistributionParameters::default();
            let mut distributor = ContentDistributor::new(params);
            
            let content = ContentItem {
                id: "test_content".to_string(),
                content_type: "post".to_string(),
                origin: GeographicLocation::new(40.7128, -74.0060).unwrap(),
                data: "test data".to_string(),
                timestamp: 1234567890,
                relevance: 1.0,
                distribution_radius: 1000.0,
            };
            
            distributor.add_content(content);
            assert!(distributor.get_content("test_content").is_some());
        }

        #[test]
        fn test_relevance_calculation() {
            let params = DistributionParameters::default();
            let distributor = ContentDistributor::new(params);
            
            let origin = GeographicLocation::new(40.7128, -74.0060).unwrap();
            let current = GeographicLocation::new(40.7130, -74.0058).unwrap();
            
            let relevance = distributor.relevance_calculator.calculate(
                "test".to_string(),
                origin,
                current,
                1.0,
                0.1,
            );
            
            assert!(relevance.relevance > 0.0);
            assert!(relevance.relevance <= 1.0);
        }
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;

    #[test]
    fn test_geographic_infrastructure_integration() {
        // Test integration between different components
        let location = GeographicLocation::new(40.7128, -74.0060).unwrap();
        let address = GeographicAddress::new(40.7128, -74.0060).unwrap();
        
        assert_eq!(address.coordinates.lat, location.lat);
        assert_eq!(address.coordinates.lon, location.lon);
    }

    #[test]
    fn test_spatial_indexing_integration() {
        let mut index = SpatialContentIndex::new();
        let metadata = ContentMetadata {
            id: "test".to_string(),
            origin: GeographicLocation::new(40.7128, -74.0060).unwrap(),
            content_type: "post".to_string(),
            timestamp: 1234567890,
            relevance: 1.0,
            gravity: None,
        };
        
        assert!(index.insert(metadata).is_ok());
        
        let results = index.query_radius(
            GeographicLocation::new(40.7130, -74.0058).unwrap(),
            1000.0,
        );
        
        assert!(!results.is_empty());
    }
}