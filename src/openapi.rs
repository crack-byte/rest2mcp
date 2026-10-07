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
                    "parameters": [
                        {
                            "name": "status",
                            "in": "query",
                            "schema": { "type": "string", "enum": ["open", "paid"], "example": "open" }
                        },
                        {
                            "name": "limit",
                            "in": "query",
                            "schema": { "type": "integer", "minimum": 1, "example": 10 }
                        }
                    ],
                    "responses": {
                        "200": { "description": "ok" }
                    }
                },
                "post": {
                    "summary": "Create invoice",
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": {
                                    "type": "object",
                                    "required": ["customer", "amount"],
                                    "properties": {
                                        "customer": { "type": "string", "example": "Acme Corp" },
                                        "amount": { "type": "number", "example": 125.5 },
                                        "currency": { "type": "string", "enum": ["USD", "EUR"], "example": "USD" }
                                    }
                                }
                            }
                        }
                    },
                    "responses": {
                        "201": { "description": "created" }
                    }
                }
            },
            "/invoices/{id}": {
                "get": {
                    "summary": "Fetch invoice",
                    "parameters": [
                        {
                            "name": "id",
                            "in": "path",
                            "required": true,
                            "schema": { "type": "integer", "example": 123 }
                        }
                    ],
                    "responses": {
                        "200": { "description": "ok" }
                    }
                },
                "delete": {
                    "summary": "Delete invoice",
                    "parameters": [
                        {
                            "name": "id",
                            "in": "path",
                            "required": true,
                            "schema": { "type": "integer", "example": 123 }
                        }
                    ],
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
