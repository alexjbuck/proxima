# Proxima: A Decentralized Geographic Social Network

Proxima is a fundamentally geographic social network where physical location forms the primary organizing principle of the network topology. Unlike traditional social networks with geographic features added on top, Proxima uses spatial proximity as the core routing, discovery, and content distribution mechanism.

## Core Philosophy: Geography-First Networking

### The Geographic Hypothesis
Human social networks are fundamentally geographic. People care most about:
1. What's happening in their immediate vicinity (neighborhood)
2. Their broader community (city/region)
3. Connections that bridge to other geographic communities

By making geography the primary network organizing principle rather than an add-on feature, Proxima creates a system that naturally reflects human social patterns and resists centralized control.

## Key Features

### Geographic Addressing
- **Every node has a geographic address** (not IP-based)
- **Multi-Scale Geographic Layers**: Hyperlocal (~100m), Neighborhood (~1km), District (~5km), City (~25km), Region (~100km)
- **Geographic routing protocol** with distance-vector routing and geographic weights

### Content Gravity Model
- **Content has geographic "mass" and "gravity"**
- **Natural geographic boundaries** - content attenuates with distance
- **Spatial content indexing** using quadtrees and geographic bloom filters

### Geographic Discovery
- **Physical proximity bootstrap** using BLE/WiFi beacons
- **Geographic anchor points** at well-known locations
- **Mobility as network infrastructure** - commuter nodes bridge regions

### Privacy and Security
- **Location privacy through ambiguity** with adaptive precision
- **K-geographic-anonymity** - never reveal location more precise than k other users
- **Geographic attack resistance** with physical presence proofs

## Installation

### Prerequisites
- Rust 1.70 or later
- Cargo

### Building
```bash
git clone <repository-url>
cd proxima
cargo build --release
```

## Usage

### Start a Node
```bash
# Start with default location (San Francisco)
cargo run -- start

# Start with custom location
cargo run -- start --location "40.7128,-74.0060"

# Start with custom configuration
cargo run -- start --config config.toml
```

### Simulate a Network
```bash
# Simulate 20 nodes for 2 minutes
cargo run -- simulate --nodes 20 --duration 120
```

### Run Tests
```bash
# Run all tests
cargo run -- test

# Run specific test types
cargo run -- test --test-type geographic
cargo run -- test --test-type routing
cargo run -- test --test-type content
```

## Configuration

Create a `config.toml` file to customize node behavior:

```toml
[network]
listen_addresses = ["127.0.0.1:0"]
bootstrap_nodes = []
max_connections = 100
connection_timeout = 30

[content]
max_content_size = 1048576  # 1MB
content_ttl = 3600          # 1 hour
cache_size = 10000

[privacy]
location_precision = 100.0  # 100m
k_anonymity = 5
enable_mixing = true

[routing]
max_hop_distance = 10000.0  # 10km
max_table_size = 10000
update_interval = 30
route_ttl = 300
max_entries_per_sector = 100
geographic_decay_factor = 0.1
social_affinity_weight = 0.3
staleness_weight = 0.2
```

## Architecture

### Core Modules

- **`geo`**: Geographic types and addressing system
- **`network`**: P2P networking layer with node discovery
- **`content`**: Content management and gravity model
- **`routing`**: Geographic routing protocol
- **`privacy`**: Privacy and security mechanisms
- **`governance`**: Geographic governance and moderation
- **`discovery`**: Network discovery and bootstrap
- **`cache`**: Spatial data structures and caching
- **`protocol`**: Network protocol definitions
- **`utils`**: Utility functions and helpers

### Geographic Layers

1. **Hyperlocal**: ~100m radius (same building/block)
2. **Neighborhood**: ~1km radius (walkable distance)
3. **District**: ~5km radius (bikeable distance)
4. **City**: ~25km radius (same metro area)
5. **Region**: ~100km radius (cultural region)

### Content Propagation

Content propagates through the network using a "gravity model":
- Content has geographic "mass" based on engagement
- Content "gravity" determines how far it spreads
- Social connections can "tunnel" content across geographic barriers
- Natural geographic boundaries emerge

## Development Roadmap

### Phase 1: Single Neighborhood (Months 1-3)
- Deploy in single dense neighborhood
- Test hyperlocal discovery and routing
- Validate geographic content dynamics

### Phase 2: Multi-Neighborhood (Months 4-6)
- Expand to adjacent neighborhoods
- Test geographic bridging via commuters
- Implement district-level routing

### Phase 3: City-Scale (Months 7-12)
- Deploy across entire metro area
- Implement full geographic hierarchy
- Test city-wide emergency broadcasts

### Phase 4: Regional Networks (Months 13-18)
- Connect multiple cities
- Test long-distance geographic routing
- Implement regional content dynamics

### Phase 5: Geographic Federation (Months 19-24)
- Enable continental-scale routing
- Implement geographic sovereignty features
- Create geographic governance tools

## Testing

Run the test suite:
```bash
cargo test
```

Run benchmarks:
```bash
cargo bench
```

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests for new functionality
5. Submit a pull request

## License

This project is licensed under the MIT License - see the LICENSE file for details.

## Acknowledgments

- Inspired by the geographic nature of human social networks
- Built with Rust for performance and safety
- Uses proven P2P networking patterns adapted for geographic routing