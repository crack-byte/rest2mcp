use serde_json::{json, Value};

use crate::mcp::ToolRegistry;

pub fn sample_openapi_spec() -> Value {
    json!({
        "openapi": "3.1.0",
        "info": {
            "title": "Example billing API",
            "version": "1.0.0"
        },
        "paths": {
            "/invoices": {
                "get": {
                    "summary": "List invoices",
                    "responses": {
                        "200": { "description": "ok" }
                    }
                },
                "post": {
                    "summary": "Create invoice",
                    "responses": {
                        "201": { "description": "created" }
                    }
                }
            },
            "/invoices/{id}": {
                "get": {
                    "summary": "Fetch invoice",
                    "responses": {
                        "200": { "description": "ok" }
                    }
                },
                "delete": {
                    "summary": "Delete invoice",
                    "responses": {
                        "200": { "description": "deleted" }
                    }
                }
            }
        }
    })
}

pub fn build_registry() -> Result<ToolRegistry, String> {
    ToolRegistry::from_openapi(&sample_openapi_spec())
}
