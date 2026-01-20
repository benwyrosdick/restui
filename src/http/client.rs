use crate::storage::{ApiRequest, AuthConfig, AuthType, HttpMethod};
use anyhow::Result;
use base64::{engine::general_purpose::STANDARD, Engine};
use reqwest::{Client, Method};
use std::time::{Duration, Instant};

/// Response from an HTTP request
#[derive(Debug, Clone)]
pub struct HttpResponse {
    pub status: u16,
    pub status_text: String,
    pub headers: Vec<(String, String)>,
    pub body: String,
    pub duration_ms: u64,
    pub size_bytes: usize,
}

impl HttpResponse {
    /// Try to format the body as pretty JSON
    pub fn pretty_body(&self) -> String {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&self.body) {
            serde_json::to_string_pretty(&json).unwrap_or_else(|_| self.body.clone())
        } else {
            self.body.clone()
        }
    }

    /// Check if the response is successful (2xx)
    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.status)
    }
}

/// HTTP client wrapper
#[derive(Clone)]
pub struct HttpClient {
    client: Client,
}

impl HttpClient {
    pub fn new() -> Result<Self> {
        let client = Client::builder().timeout(Duration::from_secs(30)).build()?;
        Ok(Self { client })
    }

    /// Execute an API request
    pub async fn execute(
        &self,
        request: &ApiRequest,
        interpolate: impl Fn(&str) -> String,
    ) -> Result<HttpResponse> {
        let url = interpolate(&request.url);
        let method = match request.method {
            HttpMethod::Get => Method::GET,
            HttpMethod::Post => Method::POST,
            HttpMethod::Put => Method::PUT,
            HttpMethod::Delete => Method::DELETE,
        };

        let mut builder = self.client.request(method, &url);

        // Add query parameters
        let query_params: Vec<(String, String)> = request
            .query_params
            .iter()
            .filter(|kv| kv.enabled && !kv.key.is_empty())
            .map(|kv| (interpolate(&kv.key), interpolate(&kv.value)))
            .collect();
        if !query_params.is_empty() {
            builder = builder.query(&query_params);
        }

        // Add headers
        for header in &request.headers {
            if header.enabled && !header.key.is_empty() {
                builder = builder.header(interpolate(&header.key), interpolate(&header.value));
            }
        }

        // Add authentication
        builder = self.apply_auth(builder, &request.auth, &interpolate);

        // Add body for POST/PUT
        if matches!(request.method, HttpMethod::Post | HttpMethod::Put) && !request.body.is_empty()
        {
            let body = interpolate(&request.body);
            builder = builder.body(body);
        }

        // Execute the request
        let start = Instant::now();
        let response = builder.send().await?;
        let duration_ms = start.elapsed().as_millis() as u64;

        // Parse response
        let status = response.status().as_u16();
        let status_text = response
            .status()
            .canonical_reason()
            .unwrap_or("")
            .to_string();
        let headers: Vec<(String, String)> = response
            .headers()
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
            .collect();

        let body = response.text().await?;
        let size_bytes = body.len();

        Ok(HttpResponse {
            status,
            status_text,
            headers,
            body,
            duration_ms,
            size_bytes,
        })
    }

    fn apply_auth(
        &self,
        builder: reqwest::RequestBuilder,
        auth: &AuthConfig,
        interpolate: &impl Fn(&str) -> String,
    ) -> reqwest::RequestBuilder {
        match auth.auth_type {
            AuthType::None => builder,
            AuthType::Bearer => {
                let token = interpolate(&auth.bearer_token);
                builder.header("Authorization", format!("Bearer {}", token))
            }
            AuthType::Basic => {
                let username = interpolate(&auth.basic_username);
                let password = interpolate(&auth.basic_password);
                let credentials = format!("{}:{}", username, password);
                let encoded = STANDARD.encode(credentials.as_bytes());
                builder.header("Authorization", format!("Basic {}", encoded))
            }
            AuthType::ApiKey => {
                let key_name = interpolate(&auth.api_key_name);
                let key_value = interpolate(&auth.api_key_value);
                if auth.api_key_location == "query" {
                    builder.query(&[(key_name, key_value)])
                } else {
                    builder.header(key_name, key_value)
                }
            }
        }
    }
}

impl Default for HttpClient {
    fn default() -> Self {
        Self::new().expect("Failed to create HTTP client")
    }
}
