use std::{env, sync::{Arc, RwLock}};

use reqwest::{Client, Method, Url};
use serde_json::{json, Map, Value};

#[derive(Debug, Clone)]
pub struct HttpClient {
    base_url: Arc<RwLock<String>>,
    client: Client,
}

impl HttpClient {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: Arc::new(RwLock::new(base_url.into().trim_end_matches('/').to_string())),
            client: Client::new(),
        }
    }

    pub fn from_env() -> Self {
        let base_url = env::var("REST2MCP_API_BASE_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:8080".to_string());
        Self::new(base_url)
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

        let path_template = tool.path.clone();
        let mut path = tool.path.clone();
        for (key, value) in params_obj {
            if matches!(key.as_str(), "name" | "method" | "path" | "body" | "headers") {
                continue;
            }

            let token = format!("{{{key}}}");
            if path_template.contains(&token) {
                let replacement = value_as_string(value);
                path = path.replace(&token, &replacement);
            }
        }

        let base_url = self.base_url();
        let url = format!(
            "{}{}",
            base_url,
            if path.starts_with('/') { path.to_string() } else { format!("/{path}") }
        );

        let mut query_pairs = Vec::new();
        for (key, value) in params_obj {
            if matches!(key.as_str(), "name" | "method" | "path" | "body" | "headers") {
                continue;
            }

            let token = format!("{{{key}}}");
            if path_template.contains(&token) {
                continue;
            }

            query_pairs.push((key.clone(), value_as_string(value)));
        }

        let mut parsed_url = Url::parse(&url).map_err(|err| format!("invalid backend URL '{url}': {err}"))?;
        for (key, value) in query_pairs {
            parsed_url.query_pairs_mut().append_pair(&key, &value);
        }

        let mut request = self.client.request(method, parsed_url);

        if let Some(Value::Object(headers)) = params.get("headers") {
            for (key, value) in headers {
                let header_value = value_as_string(value);
                request = request.header(key, header_value);
            }
        }

        let body = if let Some(body_value) = params.get("body") {
            body_value.clone()
        } else {
            let mut payload = Map::new();
            for (key, value) in params_obj {
                if matches!(key.as_str(), "name" | "method" | "path" | "headers" | "body") {
                    continue;
                }

                let token = format!("{{{key}}}");
                if path_template.contains(&token) {
                    continue;
                }

                if tool.method.eq_ignore_ascii_case("get") || tool.method.eq_ignore_ascii_case("delete") {
                    continue;
                }

                payload.insert(key.clone(), value.clone());
            }

            if payload.is_empty() {
                Value::Null
            } else {
                Value::Object(payload)
            }
        };

        if !matches!(body, Value::Null)
            && !tool.method.eq_ignore_ascii_case("get")
            && !tool.method.eq_ignore_ascii_case("delete")
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

fn value_as_string(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Bool(flag) => flag.to_string(),
        Value::Number(number) => number.to_string(),
        Value::Null => String::new(),
        _ => serde_json::to_string(value).unwrap_or_else(|_| value.to_string()),
    }
}
