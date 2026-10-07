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
                if !matches!(method_name.as_str(), "get" | "post" | "put" | "patch" | "delete" | "head" | "options") {
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
                    spec,
                );
                let output_schema = build_output_schema(operation, spec);

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
        for pair in tools.windows(2) {
            if pair[0].name == pair[1].name {
                return Err(format!("OpenAPI operations generate duplicate MCP tool name '{}'", pair[0].name));
            }
        }

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

fn build_input_schema(
    operation: &Value,
    path: &str,
    method: &str,
    path_parameters: Option<&Value>,
    spec: &Value,
) -> Value {
    let mut properties = serde_json::Map::new();
    properties.insert("path".to_string(), json!({"type": "string", "description": path}));
    properties.insert("method".to_string(), json!({"type": "string", "enum": [method]}));
    let mut required = vec!["method".to_string(), "path".to_string()];

    for parameters in [path_parameters, operation.get("parameters")] {
        if let Some(parameters) = parameters.and_then(Value::as_array) {
            for parameter in parameters {
                let resolved_parameter = resolve_schema_refs(parameter, spec, &mut Vec::new(), 0);
                let parameter = &resolved_parameter;
                if let Some(name) = parameter.get("name").and_then(Value::as_str) {
                    let schema = parameter
                        .get("schema")
                        .cloned()
                        .unwrap_or_else(|| {
                            let mut legacy = serde_json::Map::new();
                            for key in ["type", "format", "items", "enum", "default", "minimum", "maximum", "minLength", "maxLength", "pattern"] {
                                if let Some(value) = parameter.get(key) {
                                    legacy.insert(key.to_string(), value.clone());
                                }
                            }
                                Value::Object(legacy)
                        });
                            let mut schema = resolve_schema_refs(&schema, spec, &mut Vec::new(), 0);
                    if !schema.is_object() {
                        schema = json!({ "type": "string" });
                    }
                    if let Some(schema) = schema.as_object_mut() {
                        schema.insert(
                            "x-mcp-in".to_string(),
                            parameter.get("in").cloned().unwrap_or_else(|| json!("query")),
                        );
                    }
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

    let resolved_request_body = operation
        .get("requestBody")
        .map(|body| resolve_schema_refs(body, spec, &mut Vec::new(), 0));
    let request_body = resolved_request_body.as_ref();
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
        properties.insert("body".to_string(), resolve_schema_refs(schema, spec, &mut Vec::new(), 0));
        if request_body
            .and_then(|body| body.get("required"))
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            required.push("body".to_string());
        }
    } else if let Some(parameters) = operation.get("parameters").and_then(Value::as_array) {
        if let Some(body_parameter) = parameters.iter().find(|parameter| {
            parameter.get("in").and_then(Value::as_str) == Some("body")
        }) {
            let schema = body_parameter
                .get("schema")
                .cloned()
                .unwrap_or_else(|| json!({ "type": "object" }));
            let mut schema = resolve_schema_refs(&schema, spec, &mut Vec::new(), 0);
            if let Some(schema) = schema.as_object_mut() {
                schema.insert("x-mcp-in".to_string(), json!("body"));
            }
            properties.insert("body".to_string(), schema);
            if body_parameter.get("required").and_then(Value::as_bool).unwrap_or(false) {
                required.push("body".to_string());
            }
        }
    }

    json!({
        "type": "object",
        "properties": properties,
        "required": required
    })
}

fn resolve_schema_refs(schema: &Value, root: &Value, visited: &mut Vec<String>, depth: usize) -> Value {
    if depth >= 32 {
        return schema.clone();
    }

    if let Some(reference) = schema.get("$ref").and_then(Value::as_str) {
        if visited.iter().any(|seen| seen == reference) {
            return schema.clone();
        }
        if let Some(target) = reference
            .strip_prefix('#')
            .and_then(|pointer| root.pointer(pointer))
        {
            visited.push(reference.to_string());
            let mut resolved = resolve_schema_refs(target, root, visited, depth + 1);
            visited.pop();
            if let (Some(resolved), Some(siblings)) = (resolved.as_object_mut(), schema.as_object()) {
                for (key, value) in siblings {
                    if key != "$ref" {
                        resolved.insert(key.clone(), resolve_schema_refs(value, root, visited, depth + 1));
                    }
                }
            }
            return resolved;
        }
        return schema.clone();
    }

    match schema {
        Value::Object(object) => Value::Object(
            object
                .iter()
                .map(|(key, value)| (key.clone(), resolve_schema_refs(value, root, visited, depth + 1)))
                .collect(),
        ),
        Value::Array(values) => Value::Array(
            values
                .iter()
                .map(|value| resolve_schema_refs(value, root, visited, depth + 1))
                .collect(),
        ),
        _ => schema.clone(),
    }
}

fn build_output_schema(operation: &Value, spec: &Value) -> Value {
    let empty_response = json!({});
    let response = operation
        .get("responses")
        .and_then(|responses| responses.get("200"))
        .or_else(|| operation.get("responses").and_then(|responses| responses.get("201")))
        .or_else(|| operation.get("responses").and_then(|responses| responses.get("default")))
        .unwrap_or(&empty_response);
    let response = resolve_schema_refs(response, spec, &mut Vec::new(), 0);

    let response_schema = response
        .get("content")
        .and_then(Value::as_object)
        .and_then(|content| content.get("application/json").or_else(|| content.values().next()))
        .and_then(|media_type| media_type.get("schema"))
        .or_else(|| response.get("schema"));
    let body_schema = response_schema
        .map(|schema| resolve_schema_refs(schema, spec, &mut Vec::new(), 0))
        .unwrap_or_else(|| json!({ "type": "object" }));

    json!({
        "type": "object",
        "properties": {
            "tool": { "type": "string" },
            "path": { "type": "string" },
            "method": { "type": "string" },
            "risk": { "type": "string" },
            "status": { "type": "string", "enum": ["executed", "approval_required"] },
            "http_response": {
                "type": "object",
                "properties": {
                    "status": { "type": "integer" },
                    "body": body_schema
                }
            },
            "approval_token": { "type": "string" }
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
