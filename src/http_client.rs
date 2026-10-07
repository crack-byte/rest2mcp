use std::{env, sync::{Arc, RwLock}, time::Duration};

use reqwest::{Client, Method, Url};
use serde_json::{json, Map, Value};

#[derive(Clone)]
pub struct HttpClient {
    base_url: Arc<RwLock<String>>,
    client: Client,
    backend_bearer_token: Option<String>,
}

impl HttpClient {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self::new_with_timeout(base_url, Duration::from_secs(30))
    }

    pub fn new_with_timeout(base_url: impl Into<String>, timeout: Duration) -> Self {
        Self {
            base_url: Arc::new(RwLock::new(base_url.into().trim_end_matches('/').to_string())),
            client: Client::builder()
                .timeout(timeout)
                .build()
                .expect("HTTP client configuration must be valid"),
            backend_bearer_token: env::var("REST2MCP_BACKEND_BEARER_TOKEN").ok().filter(|value| !value.is_empty()),
        }
    }

    pub fn from_env() -> Self {
        Self::from_env_with_timeout(Duration::from_secs(30))
    }

    pub fn from_env_with_timeout(timeout: Duration) -> Self {
        let base_url = env::var("REST2MCP_API_BASE_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:8080".to_string());
        Self::new_with_timeout(base_url, timeout)
    }

    pub fn base_url(&self) -> String {
        self.base_url.read().expect("API base URL lock poisoned").clone()
    }

    pub fn set_base_url(&self, base_url: &str) -> Result<String, String> {
        let normalized = base_url.trim().trim_end_matches('/');
        let parsed = Url::parse(normalized)
            .map_err(|_| "Enter a valid absolute API URL, such as http://127.0.0.1:8080".to_string())?;
        if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() {
            return Err("API base URL must use http:// or https:// and include a host".to_string());
        }
        if parsed.query().is_some() || parsed.fragment().is_some() {
            return Err("API base URL cannot contain a query string or fragment".to_string());
        }

        let mut current = self.base_url.write().expect("API base URL lock poisoned");
        *current = normalized.to_string();
        Ok(current.clone())
    }

    pub async fn execute(
        &self,
        tool: &crate::mcp::ToolDefinition,
        params: &Value,
    ) -> Result<Value, String> {
        let method = tool.method.parse::<Method>().map_err(|err| err.to_string())?;
        let empty_params = Map::new();
        let params_obj = params.as_object().unwrap_or(&empty_params);
        let properties = tool.input_schema.get("properties").and_then(Value::as_object);
        if let Some(required) = tool.input_schema.get("required").and_then(Value::as_array) {
            for required in required.iter().filter_map(Value::as_str) {
                if !matches!(required, "method" | "path")
                    && !params_obj.contains_key(required)
                {
                    return Err(format!("missing required argument '{required}' for tool '{}'", tool.name));
                }
            }
        }

        let mut parsed_url = Url::parse(&self.base_url())
            .map_err(|err| format!("invalid backend base URL: {err}"))?;
        {
            let mut segments = parsed_url
                .path_segments_mut()
                .map_err(|_| "backend base URL cannot be a base URL".to_string())?;
            segments.pop_if_empty();
            for segment in tool.path.split('/').filter(|segment| !segment.is_empty()) {
                if segment.starts_with('{') && segment.ends_with('}') {
                    let name = &segment[1..segment.len() - 1];
                    let value = params_obj
                        .get(name)
                        .ok_or_else(|| format!("missing path parameter '{name}' for tool '{}'", tool.name))?;
                    segments.push(&value_as_string(value));
                } else {
                    segments.push(segment);
                }
            }
        }

        let mut request_headers = reqwest::header::HeaderMap::new();
        let mut body_parameters = Map::new();
        let mut cookie_pairs = Vec::new();
        for (key, value) in params_obj {
            if matches!(key.as_str(), "name" | "method" | "path" | "headers" | "body") {
                continue;
            }
            let location = properties
                .and_then(|properties| properties.get(key))
                .and_then(|schema| schema.get("x-mcp-in"))
                .and_then(Value::as_str)
                .unwrap_or_else(|| {
                    if tool.path.split('/').any(|segment| segment == format!("{{{key}}}")) { "path" } else { "query" }
                });
            match location {
                "path" => {}
                "query" => append_query_value(&mut parsed_url, key, value),
                "header" => {
                    let header_name = reqwest::header::HeaderName::from_bytes(key.as_bytes())
                        .map_err(|err| format!("invalid header parameter '{key}': {err}"))?;
                    let header_value = reqwest::header::HeaderValue::from_str(&value_as_string(value))
                        .map_err(|err| format!("invalid value for header parameter '{key}': {err}"))?;
                    request_headers.append(header_name, header_value);
                }
                "cookie" => cookie_pairs.push(format!("{key}={}", value_as_string(value))),
                "body" => {
                    if let Value::Object(body) = value {
                        body_parameters.extend(body.clone());
                    }
                }
                _ => return Err(format!("unsupported parameter location '{location}' for '{key}'")),
            }
        }

        if let Some(Value::Object(headers)) = params.get("headers") {
            for (key, value) in headers {
                let header_name = reqwest::header::HeaderName::from_bytes(key.as_bytes())
                    .map_err(|err| format!("invalid header name '{key}': {err}"))?;
                let header_value = reqwest::header::HeaderValue::from_str(&value_as_string(value))
                    .map_err(|err| format!("invalid value for header '{key}': {err}"))?;
                request_headers.insert(header_name, header_value);
            }
        }
        if !cookie_pairs.is_empty() {
            request_headers.insert(
                reqwest::header::COOKIE,
                reqwest::header::HeaderValue::from_str(&cookie_pairs.join("; "))
                    .map_err(|err| format!("invalid cookie parameter: {err}"))?,
            );
        }
        if let Some(token) = &self.backend_bearer_token {
            let value = reqwest::header::HeaderValue::from_str(&format!("Bearer {token}"))
                .map_err(|err| format!("invalid configured backend bearer token: {err}"))?;
            request_headers.insert(reqwest::header::AUTHORIZATION, value);
        }

        let mut request = self.client.request(method, parsed_url).headers(request_headers);
        let body = params.get("body").cloned().unwrap_or_else(|| {
            if body_parameters.is_empty() {
                let mut flattened = Map::new();
                if let Some(properties) = properties {
                    for (key, value) in params_obj {
                        let location = properties
                            .get(key)
                            .and_then(|schema| schema.get("x-mcp-in"))
                            .and_then(Value::as_str)
                            .unwrap_or("query");
                        if location == "body" {
                            flattened.insert(key.clone(), value.clone());
                        }
                    }
                }
                if flattened.is_empty() { Value::Null } else { Value::Object(flattened) }
            } else {
                Value::Object(body_parameters)
            }
        });
        if !matches!(body, Value::Null)
            && !tool.method.eq_ignore_ascii_case("get")
            && !tool.method.eq_ignore_ascii_case("delete")
            && !tool.method.eq_ignore_ascii_case("head")
            && !tool.method.eq_ignore_ascii_case("options")
        {
            request = request.json(&body);
        }

        let response = request
            .send()
            .await
            .map_err(|err| format!("backend request failed for {} {}: {err}", tool.method, tool.path))?;

        let status = response.status();
        let text = response
            .text()
            .await
            .map_err(|err| format!("failed to read backend response for {} {}: {err}", tool.method, tool.path))?;

        let parsed = match serde_json::from_str::<Value>(&text) {
            Ok(value) => value,
            Err(_) => Value::String(text),
        };

        if !status.is_success() {
            return Err(format!(
                "backend {} {} returned HTTP {}: {}",
                tool.method,
                tool.path,
                status.as_u16(),
                parsed
            ));
        }

        Ok(json!({
            "status": status.as_u16(),
            "body": parsed
        }))
    }
}

fn append_query_value(url: &mut Url, key: &str, value: &Value) {
    let mut query = url.query_pairs_mut();
    if let Some(values) = value.as_array() {
        for item in values {
            query.append_pair(key, &value_as_string(item));
        }
    } else {
        query.append_pair(key, &value_as_string(value));
    }
}

fn value_as_string(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Bool(flag) => flag.to_string(),
        Value::Number(number) => number.to_string(),
        Value::Null => String::new(),
        _ => serde_json::to_string(value).unwrap_or_else(|_| value.to_string()),
    }
}
