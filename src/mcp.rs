use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::security::CallerContext;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum RiskClassification {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolDefinition {
    pub name: String,
    pub operation_id: String,
    pub description: String,
    pub method: String,
    pub path: String,
    pub risk: RiskClassification,
    pub required_scopes: Vec<String>,
    pub deprecated: bool,
    pub input_schema: Value,
    pub output_schema: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpRequest {
    pub jsonrpc: String,
    pub id: Value,
    pub method: String,
    pub params: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpResponse {
    pub jsonrpc: String,
    pub id: Value,
    pub result: Value,
}

#[derive(Debug, Clone, Default)]
pub struct ToolRegistry {
    tools: Vec<ToolDefinition>,
}

impl ToolRegistry {
    pub fn from_openapi(spec: &Value) -> Result<Self, String> {
        let paths = spec
            .get("paths")
            .and_then(Value::as_object)
            .ok_or_else(|| "OpenAPI document is missing a valid 'paths' object".to_string())?;

        let mut tools = Vec::new();

        for (path, operations) in paths {
            let operations = operations
                .as_object()
                .ok_or_else(|| format!("OpenAPI path '{path}' does not map to an object"))?;

            for (method_name, operation) in operations {
                if !matches!(method_name.as_str(), "get" | "post" | "put" | "patch" | "delete") {
                    continue;
                }

                let summary = sanitize_text(
                    operation
                        .get("summary")
                        .and_then(Value::as_str)
                        .unwrap_or("Generated from OpenAPI specification"),
                );
                let description = operation
                    .get("description")
                    .and_then(Value::as_str)
                    .map(sanitize_text)
                    .unwrap_or_else(|| summary.clone());

                let operation_id = operation
                    .get("operationId")
                    .and_then(Value::as_str)
                    .map(|value| sanitize_text(value))
                    .unwrap_or_else(|| format!("{}_{}", method_name, normalize_path(path)));

                let tool_name = sanitize_tool_name(&operation_id);
                let risk = infer_risk(method_name.as_str());
                let required_scopes = required_scopes_for_method(method_name.as_str());
                let deprecated = operation
                    .get("deprecated")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);

                let input_schema = build_input_schema(
                    operation,
                    path,
                    method_name.as_str(),
                    operations.get("parameters"),
                );
                let output_schema = build_output_schema(operation);

                tools.push(ToolDefinition {
                    name: tool_name,
                    operation_id,
                    description,
                    method: method_name.to_uppercase(),
                    path: path.clone(),
                    risk,
                    required_scopes,
                    deprecated,
                    input_schema,
                    output_schema,
                });
            }
        }

        tools.sort_by(|left, right| left.name.cmp(&right.name));

        Ok(Self { tools })
    }

    pub fn list(&self) -> &[ToolDefinition] {
        &self.tools
    }

    pub fn list_for_caller(&self, caller: &CallerContext) -> Vec<ToolDefinition> {
        self.tools
            .iter()
            .filter(|tool| caller.can_access(tool))
            .cloned()
            .collect()
    }

    pub fn find_by_name(&self, name: &str) -> Option<&ToolDefinition> {
        self.tools.iter().find(|tool| tool.name == name)
    }
}

fn sanitize_text(value: &str) -> String {
    let mut cleaned = value.to_string();
    for marker in [
        "ignore previous instructions",
        "ignore prior instructions",
        "override system prompt",
        "execute shell command",
        "run rm -rf",
    ] {
        cleaned = cleaned.replace(marker, "[sanitized]");
    }
    cleaned = cleaned.replace('\n', " ").trim().to_string();
    cleaned
}

fn sanitize_tool_name(value: &str) -> String {
    value
        .chars()
        .map(|ch| match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' => ch,
            _ => '_',
        })
        .collect::<String>()
        .trim_matches('_')
        .to_lowercase()
}

fn build_input_schema(operation: &Value, path: &str, method: &str, path_parameters: Option<&Value>) -> Value {
    let mut properties = serde_json::Map::new();
    properties.insert("path".to_string(), json!({"type": "string", "description": path}));
    properties.insert("method".to_string(), json!({"type": "string", "enum": [method]}));
    let mut required = vec!["method".to_string(), "path".to_string()];

    for parameters in [path_parameters, operation.get("parameters")] {
        if let Some(parameters) = parameters.and_then(Value::as_array) {
            for parameter in parameters {
                if let Some(name) = parameter.get("name").and_then(Value::as_str) {
                    let schema = parameter
                        .get("schema")
                        .cloned()
                        .unwrap_or_else(|| json!({ "type": "string" }));
                    properties.insert(name.to_string(), schema);
                    if parameter.get("required").and_then(Value::as_bool).unwrap_or(false)
                        && !required.iter().any(|required_name| required_name == name)
                    {
                        required.push(name.to_string());
                    }
                }
            }
        }
    }

    let request_body = operation.get("requestBody");
    let body_schema = request_body
        .and_then(|body| body.get("content"))
        .and_then(Value::as_object)
        .and_then(|content| {
            content
                .get("application/json")
                .or_else(|| content.values().next())
        })
        .and_then(|media_type| media_type.get("schema"));
    if let Some(schema) = body_schema {
        properties.insert("body".to_string(), schema.clone());
        if request_body
            .and_then(|body| body.get("required"))
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            required.push("body".to_string());
        }
    }

    json!({
        "type": "object",
        "properties": properties,
        "required": required
    })
}

fn build_output_schema(operation: &Value) -> Value {
    let empty_response = json!({});
    let response = operation
        .get("responses")
        .and_then(|responses| responses.get("200"))
        .or_else(|| operation.get("responses").and_then(|responses| responses.get("201")))
        .or_else(|| operation.get("responses").and_then(|responses| responses.get("default")))
        .unwrap_or(&empty_response);

    json!({
        "type": "object",
        "description": format!("Response schema generated from OpenAPI operation: {}", response),
        "properties": {
            "status": { "type": "string" },
            "body": { "type": "object" }
        }
    })
}

fn normalize_path(path: &str) -> String {
    let cleaned = path
        .replace('{', "")
        .replace('}', "")
        .replace('/', "_")
        .replace('-', "_")
        .trim_matches('_')
        .to_string();

    if cleaned.is_empty() {
        "root".to_string()
    } else {
        cleaned
    }
}

fn infer_risk(method: &str) -> RiskClassification {
    match method {
        "delete" => RiskClassification::Critical,
        "patch" | "put" => RiskClassification::High,
        "post" => RiskClassification::Medium,
        _ => RiskClassification::Low,
    }
}

fn required_scopes_for_method(method: &str) -> Vec<String> {
    match method {
        "delete" => vec!["admin:write".to_string()],
        "post" | "put" | "patch" => vec!["write:resources".to_string()],
        _ => vec!["read:resources".to_string()],
    }
}
