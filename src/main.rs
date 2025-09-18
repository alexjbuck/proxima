use proxima::*;
use clap::{Parser, Subcommand};
use tracing::{info, error};
use tracing_subscriber;

/// Proxima: A Decentralized Geographic Social Network
#[derive(Parser)]
#[command(name = "proxima")]
#[command(about = "A decentralized geographic social network where physical location forms the primary organizing principle")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start a Proxima node
    Start {
        /// Node configuration file
        #[arg(short, long)]
        config: Option<String>,
        /// Node location (latitude,longitude)
        #[arg(short, long)]
        location: Option<String>,
        /// Node port
        #[arg(short, long, default_value = "0")]
        port: u16,
    },
    /// Simulate a network of nodes
    Simulate {
        /// Number of nodes to simulate
        #[arg(short, long, default_value = "10")]
        nodes: usize,
        /// Simulation duration in seconds
        #[arg(short, long, default_value = "60")]
        duration: u64,
    },
    /// Test geographic functions
    Test {
        /// Test type
        #[arg(short, long)]
        test_type: Option<String>,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();
    
    let cli = Cli::parse();
    
    match cli.command {
        Commands::Start { config, location, port } => {
            start_node(config, location, port).await?;
        }
        Commands::Simulate { nodes, duration } => {
            simulate_network(nodes, duration).await?;
        }
        Commands::Test { test_type } => {
            run_tests(test_type).await?;
        }
    }
    
    Ok(())
}

/// Start a Proxima node
async fn start_node(
    config_path: Option<String>,
    location_str: Option<String>,
    port: u16,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("Starting Proxima node...");
    
    // Parse location if provided
    let location = if let Some(loc_str) = location_str {
        let parts: Vec<&str> = loc_str.split(',').collect();
        if parts.len() != 2 {
            return Err("Invalid location format. Use: latitude,longitude".into());
        }
        let lat: f64 = parts[0].parse()?;
        let lon: f64 = parts[1].parse()?;
        GeographicLocation::new(lat, lon, 50.0)?
    } else {
        // Default to San Francisco
        GeographicLocation::new(37.7749, -122.4194, 50.0)?
    };
    
    // Load configuration
    let config = if let Some(config_path) = config_path {
        NodeConfig::load_from_file(&config_path)?
    } else {
        NodeConfig::default()
    };
    
    // Create and start node
    let mut node = ProximaNode::new(location, config).await?;
    node.start().await?;
    
    info!("Proxima node started successfully!");
    info!("Node location: {}", node.location);
    info!("Node ID: {}", node.identity.id);
    
    // Keep the node running
    tokio::signal::ctrl_c().await?;
    info!("Shutting down Proxima node...");
    
    Ok(())
}

/// Simulate a network of nodes
async fn simulate_network(
    node_count: usize,
    duration_seconds: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("Starting network simulation with {} nodes for {} seconds", node_count, duration_seconds);
    
    let mut nodes = Vec::new();
    
    // Create nodes in a grid pattern around San Francisco
    let base_lat = 37.7749;
    let base_lon = -122.4194;
    let grid_size = (node_count as f64).sqrt().ceil() as usize;
    let spacing = 0.01; // Approximately 1km spacing
    
    for i in 0..node_count {
        let row = i / grid_size;
        let col = i % grid_size;
        
        let lat = base_lat + (row as f64 - grid_size as f64 / 2.0) * spacing;
        let lon = base_lon + (col as f64 - grid_size as f64 / 2.0) * spacing;
        
        let location = GeographicLocation::new(lat, lon, 10.0)?;
        let config = NodeConfig::default();
        
        let mut node = ProximaNode::new(location, config).await?;
        node.start().await?;
        
        nodes.push(node);
        info!("Created node {} at location {}", i, location);
    }
    
    // Simulate content publishing
    for (i, node) in nodes.iter_mut().enumerate() {
        let content = Content {
            id: ContentId::new(),
            author: format!("node_{}", i),
            content_type: ContentType::Text,
            data: format!("Hello from node {} at {}", i, node.location).into_bytes(),
            timestamp: chrono::Utc::now(),
            location: node.location.clone(),
            tags: vec!["simulation".to_string()],
            metadata: ContentMetadata::default(),
        };
        
        let content_id = node.publish_content(content).await?;
        info!("Node {} published content: {}", i, content_id);
    }
    
    // Wait for simulation duration
    tokio::time::sleep(tokio::time::Duration::from_secs(duration_seconds)).await;
    
    info!("Simulation completed!");
    
    Ok(())
}

/// Run tests
async fn run_tests(test_type: Option<String>) -> Result<(), Box<dyn std::error::Error>> {
    match test_type.as_deref() {
        Some("geographic") => {
            info!("Running geographic tests...");
            test_geographic_functions().await?;
        }
        Some("routing") => {
            info!("Running routing tests...");
            test_routing_functions().await?;
        }
        Some("content") => {
            info!("Running content tests...");
            test_content_functions().await?;
        }
        Some("all") | None => {
            info!("Running all tests...");
            test_geographic_functions().await?;
            test_routing_functions().await?;
            test_content_functions().await?;
        }
        _ => {
            error!("Unknown test type: {:?}", test_type);
            return Err("Unknown test type".into());
        }
    }
    
    info!("All tests completed successfully!");
    Ok(())
}

/// Test geographic functions
async fn test_geographic_functions() -> Result<(), Box<dyn std::error::Error>> {
    info!("Testing geographic functions...");
    
    // Test location creation
    let location = GeographicLocation::new(37.7749, -122.4194, 10.0)?;
    info!("Created location: {}", location);
    
    // Test distance calculation
    let location2 = GeographicLocation::new(37.7849, -122.4094, 10.0)?;
    let distance = location.distance_to(&location2);
    info!("Distance between locations: {:.2} meters", distance);
    
    // Test geographic addressing
    let address = GeographicAddress::new(37.7749, -122.4194, GeographicLayer::Neighborhood, 0.9)?;
    info!("Created address: {:?}", address);
    
    // Test geographic sectors
    let sector = GeographicSector::from_relative_location(&location, &location2);
    info!("Geographic sector: {:?}", sector);
    
    Ok(())
}

/// Test routing functions
async fn test_routing_functions() -> Result<(), Box<dyn std::error::Error>> {
    info!("Testing routing functions...");
    
    // Test routing table creation
    let routing_table = RoutingTable::new();
    info!("Created routing table");
    
    // Test route cost calculation
    let cost = routing_table.calculate_route_cost(1000.0, 0.8, 0.9);
    info!("Route cost: {:.2}", cost);
    
    Ok(())
}

/// Test content functions
async fn test_content_functions() -> Result<(), Box<dyn std::error::Error>> {
    info!("Testing content functions...");
    
    // Test content creation
    let location = GeographicLocation::new(37.7749, -122.4194, 10.0)?;
    let content = Content {
        id: ContentId::new(),
        author: "test_user".to_string(),
        content_type: ContentType::Text,
        data: "Hello, Proxima!".as_bytes().to_vec(),
        timestamp: chrono::Utc::now(),
        location: location.clone(),
        tags: vec!["test".to_string()],
        metadata: ContentMetadata::default(),
    };
    
    info!("Created content: {}", content.id);
    
    // Test content gravity calculation
    let gravity_calculator = ContentGravityCalculator::new();
    let gravity = gravity_calculator.calculate_gravity(&content, &location, &std::collections::HashSet::new());
    info!("Content gravity: {:.3}", gravity.combined_relevance);
    
    Ok(())
}

/// Default node configuration
impl Default for NodeConfig {
    fn default() -> Self {
        Self {
            network: NetworkConfig {
                listen_addresses: vec!["127.0.0.1:0".to_string()],
                bootstrap_nodes: vec![],
                max_connections: 100,
                connection_timeout: std::time::Duration::from_secs(30),
            },
            content: ContentConfig {
                max_content_size: 1024 * 1024, // 1MB
                content_ttl: std::time::Duration::from_secs(3600), // 1 hour
                cache_size: 10000,
            },
            privacy: PrivacyConfig {
                location_precision: 100.0,
                k_anonymity: 5,
                enable_mixing: true,
            },
            routing: RoutingConfig::default(),
        }
    }
}

/// Load configuration from file
impl NodeConfig {
    fn load_from_file(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        let config: NodeConfig = toml::from_str(&content)?;
        Ok(config)
    }
}
