# Worldgen - A Set of Tools for Traveller

Worldgen started as a world generator, but has evolved into a set of tools for Traveller. Built entirely in Rust using the Leptos reactive framework, it currently provides two primary tools for Traveller RPG players and referees.

## Overview

Worldgen combines stellar mechanics, world generation, and trade economics into tools for the Traveller universe. The application generates realistic star systems following official Traveller rules while providing modern web-based interfaces for system development and the trade mini-game.  The tools are hosted at [http://worldgen.callisot.com]

## Key Features

### Solar System Generator

- **Complete Star Systems**: Generates primary stars with up to two companion stars
- **Orbital Mechanics**: Creates realistic orbital arrangements for worlds and gas giants
- **World Generation**: Full Universal World Profile (UWP) generation with trade classifications
- **Satellite Systems**: Recursive moon and satellite generation for complex systems
- **Astronomical Data**: Detailed physical characteristics and orbital information

### Trade Computer

- **Market Generation**: Dynamic trade goods based on world characteristics
- **Route Planning**: Calculate trade opportunities between worlds
- **Ship Manifests**: Passenger and freight management with profit/loss analysis
- **Traveller Map Integration**: Official universe data import and coordinate systems

### Interactive Features

- **Real-time Updates**: Reactive interface updates as parameters change
- **Traveller Map API**: Search and import canonical world data
- **Export Capabilities**: Share generated systems and trade data
- **Responsive Design**: Works across desktop and mobile devices

## Technology Stack

### Core Framework

- **Rust**: Systems programming language for performance and safety
- **Leptos**: Reactive web framework with embedded HTML/CSS
- **WebAssembly**: Client-side execution for fast, responsive interfaces

### Key Dependencies

- **reactive_stores**: Complex state management for nested data structures
- **wasm_logger**: Browser console logging with URL parameter control

## Getting Started

### Prerequisites

You'll need the following tools installed:

1. **Rust and Cargo**: Install from [rustup.rs](https://rustup.rs/)
2. **WebAssembly Target**: Required for browser compilation
3. **Trunk**: Build tool for Rust web applications

### Installation

1. Install the WebAssembly compilation target:

   ```bash
   rustup target add wasm32-unknown-unknown
   ```

2. Install Trunk build tool:

   ```bash
   cargo install trunk
   ```

3. Clone and run the application:

   ```bash
   git clone <repository-url>
   cd worldgen
   trunk serve --open
   ```

This will compile the application, start a development server, and open your browser to the running application.

## Configuration

### `TRAVELLERMAP_URL` — override the upstream TravellerMap host

By default the WASM frontend and the native backend both talk to
`https://travellermap.com` for sector/world lookups, search, and tile
rendering. If you're running your own TravellerMap-compatible service
(self-hosted instance, staging mirror, etc.), set `TRAVELLERMAP_URL`
**at build time** to override it:

```bash
# Local dev — the same export covers both binaries
export TRAVELLERMAP_URL=https://my.tmap.local
trunk serve              # frontend
cargo run --features backend --bin server   # backend (in another terminal)

# Verify what got baked into the binary
cargo run --example show_travellermap_url
# →  travellermap_base_url() = "https://my.tmap.local"
```

Important details:

- **It's a *build-time* setting**, not a runtime one. The URL gets
  baked into both binaries via `option_env!` so there's no per-request
  configuration to forget on either side. Change the value and
  rebuild.
- **Single source of truth.** The same env var name is read by both
  the WASM bundle and the native server, so you can't have them point
  at different hosts by accident.
- **Trailing slashes are stripped** — `https://x.com` and
  `https://x.com/` both work.
- **Unset = default.** Omitting the variable falls back to
  `https://travellermap.com`, so the public deploy needs no extra
  configuration.

For Docker / Cloud Run deployments, see the [Docker
Deployment](#docker-deployment) section — the build script forwards
the var as a `--build-arg` automatically.

## Application Structure

### URL Routing

- **/** - Main selector interface for choosing tools
- **/world** - Solar system generator interface
- **/trade** - Trade computer and route planning

### Component Architecture

- **Selector**: Main application entry point and tool selection
- **System Generator**: Complete star system creation interface
- **Trade Computer**: Market analysis and route planning tools
- **World List**: Tabular display of system objects and characteristics
- **System View**: Visual representation of generated star systems
- **Traveller Map**: Integration with official universe data

## Usage Examples

### Basic System Generation

1. Navigate to the system generator (/world)
2. Enter a world name and UWP (e.g., "Regina A788899-A")
3. View the generated star system with orbital details
4. Export or share the system data

### Trade Route Planning

1. Navigate to the trade computer (/trade)
2. Select source and destination worlds
3. Review available goods and passenger opportunities
4. Build ship manifest and calculate profitability

### Debug Logging

Enable detailed logging through URL parameters:

```text
# Debug system generation
http://localhost:8080/world?log=debug&module=worldgen::systems

# Trace trade calculations
http://localhost:8080/trade?log=trace&module=worldgen::trade
```

## Development

### Project Structure

```text
src/
├── components/          # UI components and interfaces
│   ├── selector.rs     # Main application selector
│   ├── system_generator.rs  # System generation interface
│   ├── trade_computer.rs    # Trade calculation interface
│   ├── world_list.rs   # Tabular system display
│   ├── system_view.rs  # Visual system representation
│   └── traveller_map.rs     # External API integration
├── systems/            # Core generation logic
│   ├── system.rs       # Main system coordination
│   ├── world.rs        # World generation and UWP handling
│   ├── gas_giant.rs    # Gas giant characteristics
│   └── astro.rs        # Astronomical calculations
├── trade/              # Trade and economic systems
│   ├── available_goods.rs   # Market generation
│   └── ship_manifest.rs     # Cargo and passenger management
├── logging.rs          # URL-based logging configuration
└── lib.rs              # Main library interface
```

### Building for Production

```bash
trunk build --release
```

To bake in a custom TravellerMap host, set `TRAVELLERMAP_URL` before
the build (see [Configuration](#configuration)).

### Docker Deployment

```bash
docker build -t worldgen .
docker run -p 8080:80 worldgen
```

To deploy to the project's Cloud Run instance, use the helper script:

```bash
# Optional: override the TravellerMap host before pushing
export TRAVELLERMAP_URL=https://my.tmap.local

./scripts/push_image.sh
```

`push_image.sh` forwards `TRAVELLERMAP_URL` to `docker buildx` as a
`--build-arg` so the value gets baked into both the WASM bundle and
the native server inside the image. It also prompts for `GCS_BUCKET`
(the planet-PNG cache for the `/world` endpoint) if it's not set.

## Contributing

Worldgen follows standard Rust development practices:

- Use `cargo fmt` for code formatting
- Run `cargo clippy` for linting
- Add tests for new functionality
- Update documentation for API changes

## License

License is the MIT License, see [LICENSE](LICENSE)

## Acknowledgments

- **Traveller RPG**: Classic science fiction role-playing game by Marc Miller. All rights owned by Mongoose Publishing.
- **Traveller Map**: Official universe mapping service and API
- **Leptos Community**: Reactive web framework development and support
