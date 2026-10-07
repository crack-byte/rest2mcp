# REST2MCP

A learning practice project for converting REST APIs to MCP (Model Context Protocol) servers.

This project demonstrates how to create a gateway that transforms RESTful APIs into MCP-compatible services, enabling AI models to interact with existing REST endpoints through the standardized MCP interface.

## Features

- OpenAPI specification parsing
- REST to MCP transformation
- Security handling
- Configuration management
- Axum-based web server implementation

## Project Structure

```
REST2MCP/
├── Cargo.toml
├── Cargo.lock
├── README.md
├── .gitignore
├── .vscode/
├── docs/
│   ├── README.md
│   ├── architecture.md
│   ├── openapi_to_mcp_gateway_requirements.md
│   └── security.md
├── src/
│   ├── main.rs
│   ├── config.rs
│   ├── mcp.rs
│   ├── openapi.rs
│   └── security.rs
└── target/
```

## Learning Objectives

This project serves as a practical learning exercise in:
- Rust programming with async/await patterns
- Web framework usage (Axum)
- API specification parsing (OpenAPI/Swagger)
- Protocol translation and adaptation
- Security implementation in Rust services
- Project organization and documentation

## Getting Started

1. Clone the repository
2. Install Rust toolchain if not already installed
3. Run `cargo run` to start the server
4. Configure your OpenAPI specification in the config files
5. Access the MCP endpoint to interact with your REST API through the MCP interface

## Documentation

Detailed documentation can be found in the `docs/` directory:
- [Architecture Overview](docs/architecture.md)
- [OpenAPI to MCP Gateway Requirements](docs/openapi_to_mcp_gateway_requirements.md)
- [Security Considerations](docs/security.md)

## License

This is a learning practice project. Feel free to use it for educational purposes.