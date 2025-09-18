use proxima::*;
use clap::{Parser, Subcommand};
use tracing::info;
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
        /// Node location (latitude,longitude)
        #[arg(short, long)]
        location: Option<String>,
    },
    /// Test basic functionality
    Test,
}

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();
    
    let cli = Cli::parse();
    
    match cli.command {
        Commands::Start { location } => {
            start_node(location).await?;
        }
        Commands::Test => {
            run_tests().await?;
        }
    }
    
    Ok(())
}

/// Start a Proxima node
async fn start_node(location_str: Option<String>) -> std::result::Result<(), Box<dyn std::error::Error>> {
    info!("Starting Proxima node...");
    
    // Parse location if provided
    let (lat, lon) = if let Some(loc_str) = location_str {
        let parts: Vec<&str> = loc_str.split(',').collect();
        if parts.len() != 2 {
            return Err("Invalid location format. Use: latitude,longitude".into());
        }
        let lat: f64 = parts[0].parse()?;
        let lon: f64 = parts[1].parse()?;
        (lat, lon)
    } else {
        // Default to San Francisco
        (37.7749, -122.4194)
    };
    
    // Create node
    let node = ProximaNode::new(lat, lon)?;
    
    info!("Proxima node started successfully!");
    info!("Node ID: {}", node.id());
    info!("Node location: {}", node.location());
    
    // Keep the node running
    tokio::signal::ctrl_c().await?;
    info!("Shutting down Proxima node...");
    
    Ok(())
}

/// Run basic tests
async fn run_tests() -> std::result::Result<(), Box<dyn std::error::Error>> {
    info!("Running basic tests...");
    
    // Test node creation
    let node = ProximaNode::new(37.7749, -122.4194)?;
    info!("Created node with ID: {}", node.id());
    
    // Test distance calculation
    let node2 = ProximaNode::new(37.7849, -122.4094)?;
    let distance = node.distance_to(&node2);
    info!("Distance between nodes: {:.2} meters", distance);
    
    // Test content publishing
    let mut node_with_content = ProximaNode::new(37.7749, -122.4194)?;
    let location = GeographicLocation::new(37.7749, -122.4194, 10.0)?;
    
    let content = Content::new(
        "test_user".to_string(),
        ContentType::Text,
        "Hello from Proxima!".as_bytes().to_vec(),
        location,
        vec!["demo".to_string()],
    );
    
    let content_id = node_with_content.publish_content(content)?;
    info!("Published content: {}", content_id);
    info!("Node now has {} pieces of content", node_with_content.content_count());
    
    info!("All tests completed successfully!");
    Ok(())
}
