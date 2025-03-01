# Swarm Skirmish Development Guide

## Build Commands
```bash
cargo build                  # Build all workspace members
cargo build --bin server     # Build server only
cargo build --bin simple-bot # Build bot client only
cargo run --bin server       # Run the server
cargo run --bin simple-bot   # Run the bot client
cargo test                   # Run all tests
cargo test -p server         # Run server tests only
cargo test <test_name>       # Run a specific test
cargo clippy                 # Run linter
cargo fmt                    # Format code
```

## Code Style Guidelines
- **Formatting**: Use `rustfmt` with the project's `rustfmt.toml` (80 char line limit)
- **Imports**: Group by Std/External/Crate with StdExternalCrate ordering
- **Error Handling**: Use `eyre` for application errors, `thiserror` for library errors
- **Types**: Always add type annotations to public interfaces
- **Documentation**: Document public APIs with doc comments (///), include examples
- **Naming**: Use snake_case for variables/functions, CamelCase for types/enums
- **Components**: Add `#[derive(Component)]` for ECS entities in the server
- **Serialization**: Add `#[derive(Serialize, Deserialize)]` for protocol messages