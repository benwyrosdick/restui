use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// HTTP methods supported by the application
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "UPPERCASE")]
pub enum HttpMethod {
    #[default]
    Get,
    Post,
    Put,
    Patch,
    Delete,
}

impl HttpMethod {
    pub fn as_str(&self) -> &'static str {
        match self {
            HttpMethod::Get => "GET",
            HttpMethod::Post => "POST",
            HttpMethod::Put => "PUT",
            HttpMethod::Patch => "PATCH",
            HttpMethod::Delete => "DELETE",
        }
    }

    pub fn all() -> &'static [HttpMethod] {
        &[
            HttpMethod::Get,
            HttpMethod::Post,
            HttpMethod::Put,
            HttpMethod::Patch,
            HttpMethod::Delete,
        ]
    }

    pub fn next(&self) -> HttpMethod {
        match self {
            HttpMethod::Get => HttpMethod::Post,
            HttpMethod::Post => HttpMethod::Put,
            HttpMethod::Put => HttpMethod::Patch,
            HttpMethod::Patch => HttpMethod::Delete,
            HttpMethod::Delete => HttpMethod::Get,
        }
    }

    pub fn prev(&self) -> HttpMethod {
        match self {
            HttpMethod::Get => HttpMethod::Delete,
            HttpMethod::Post => HttpMethod::Get,
            HttpMethod::Put => HttpMethod::Post,
            HttpMethod::Patch => HttpMethod::Put,
            HttpMethod::Delete => HttpMethod::Patch,
        }
    }
}

impl std::fmt::Display for HttpMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Key-value pair for headers and query params
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct KeyValue {
    pub key: String,
    pub value: String,
    pub enabled: bool,
}

impl KeyValue {
    pub fn new(key: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: value.into(),
            enabled: true,
        }
    }
}

/// Authentication type
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AuthType {
    #[default]
    None,
    Bearer,
    Basic,
    ApiKey,
}

impl AuthType {
    pub fn as_str(&self) -> &'static str {
        match self {
            AuthType::None => "None",
            AuthType::Bearer => "Bearer Token",
            AuthType::Basic => "Basic Auth",
            AuthType::ApiKey => "API Key",
        }
    }

    pub fn all() -> &'static [AuthType] {
        &[
            AuthType::None,
            AuthType::Bearer,
            AuthType::Basic,
            AuthType::ApiKey,
        ]
    }

    pub fn next(&self) -> AuthType {
        match self {
            AuthType::None => AuthType::Bearer,
            AuthType::Bearer => AuthType::Basic,
            AuthType::Basic => AuthType::ApiKey,
            AuthType::ApiKey => AuthType::None,
        }
    }
}

/// Authentication configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AuthConfig {
    pub auth_type: AuthType,
    /// Bearer token value
    pub bearer_token: String,
    /// Basic auth username
    pub basic_username: String,
    /// Basic auth password
    pub basic_password: String,
    /// API key name (header name or query param name)
    pub api_key_name: String,
    /// API key value
    pub api_key_value: String,
    /// Where to send API key: "header" or "query"
    pub api_key_location: String,
}

/// Represents an API request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiRequest {
    pub id: String,
    pub name: String,
    pub method: HttpMethod,
    pub url: String,
    pub headers: Vec<KeyValue>,
    pub query_params: Vec<KeyValue>,
    pub body: String,
    pub auth: AuthConfig,
}

impl Default for ApiRequest {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name: String::from("New Request"),
            method: HttpMethod::Get,
            url: String::new(),
            headers: vec![KeyValue::new("Content-Type", "application/json")],
            query_params: Vec::new(),
            body: String::new(),
            auth: AuthConfig::default(),
        }
    }
}

impl ApiRequest {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Default::default()
        }
    }

    /// Get a display name for the request (method + path or name)
    pub fn display_name(&self) -> String {
        if self.url.is_empty() {
            format!("{} {}", self.method, self.name)
        } else {
            // Extract path from URL
            let path = self
                .url
                .split("://")
                .nth(1)
                .and_then(|s| s.split('/').skip(1).next())
                .map(|s| format!("/{}", s))
                .unwrap_or_else(|| self.url.clone());
            format!("{} {}", self.method, path)
        }
    }
}
