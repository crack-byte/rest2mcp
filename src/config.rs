use std::{env, fmt, str::FromStr};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub enum TransportMode {
    Stdio,
    #[default]
    StreamableHttp,
}

impl fmt::Display for TransportMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Stdio => write!(f, "stdio"),
            Self::StreamableHttp => write!(f, "streamable-http"),
        }
    }
}

impl FromStr for TransportMode {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "stdio" => Ok(Self::Stdio),
            "streamable-http" | "http" | "streamable_http" => Ok(Self::StreamableHttp),
            _ => Err(format!("unsupported transport mode '{value}'")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AuthConfig {
    pub dual_persona: bool,
    pub allow_human_sso: bool,
    pub allow_service_accounts: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayConfig {
    pub transport: TransportMode,
    pub http_bind: String,
    pub log_to_stderr: bool,
    pub auth: AuthConfig,
    pub ui_enabled: bool,
}

impl GatewayConfig {
    pub fn from_env() -> Self {
        let transport = env::var("REST2MCP_TRANSPORT")
            .ok()
            .and_then(|value| TransportMode::from_str(&value).ok())
            .unwrap_or_default();

        let http_bind = env::var("REST2MCP_BIND")
            .unwrap_or_else(|_| "0.0.0.0:3000".to_string());

        let log_to_stderr = env::var("REST2MCP_LOG_TO_STDERR")
            .map(|value| value.parse::<bool>().unwrap_or(true))
            .unwrap_or(true);

        let ui_enabled = env::var("REST2MCP_ENABLE_UI")
            .map(|value| value.parse::<bool>().unwrap_or(true))
            .unwrap_or(true);

        let auth = AuthConfig {
            dual_persona: env::var("REST2MCP_DUAL_PERSONA")
                .map(|value| value.parse::<bool>().unwrap_or(true))
                .unwrap_or(true),
            allow_human_sso: env::var("REST2MCP_ALLOW_HUMAN_SSO")
                .map(|value| value.parse::<bool>().unwrap_or(true))
                .unwrap_or(true),
            allow_service_accounts: env::var("REST2MCP_ALLOW_SERVICE_ACCOUNTS")
                .map(|value| value.parse::<bool>().unwrap_or(true))
                .unwrap_or(true),
        };

        Self {
            transport,
            http_bind,
            log_to_stderr,
            auth,
            ui_enabled,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.http_bind.trim().is_empty() {
            return Err("REST2MCP_BIND cannot be empty".to_string());
        }

        if !self.http_bind.contains(':') {
            return Err(format!("invalid bind address '{}'; expected host:port", self.http_bind));
        }

        Ok(())
    }
}

impl Default for GatewayConfig {
    fn default() -> Self {
        Self {
            transport: TransportMode::StreamableHttp,
            http_bind: "0.0.0.0:3000".to_string(),
            log_to_stderr: true,
            auth: AuthConfig {
                dual_persona: true,
                allow_human_sso: true,
                allow_service_accounts: true,
            },
            ui_enabled: true,
        }
    }
}
