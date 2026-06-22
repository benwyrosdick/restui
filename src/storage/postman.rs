use super::collection::{Collection, CollectionItem};
use super::environment::Environment;
use super::request::{ApiRequest, AuthConfig, AuthType, HttpMethod, KeyValue};
use anyhow::{bail, Result};
use serde::Deserialize;

/// Result of importing a Postman collection.
pub struct ImportResult {
    pub collection: Collection,
    pub environment: Option<Environment>,
    pub request_count: usize,
}

// --- Postman v2.x schema (only the fields we use) ---

#[derive(Debug, Deserialize)]
struct PostmanCollection {
    #[serde(default)]
    info: PostmanInfo,
    #[serde(default)]
    item: Vec<PostmanItem>,
    #[serde(default)]
    variable: Vec<PostmanVariable>,
}

#[derive(Debug, Default, Deserialize)]
struct PostmanInfo {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    schema: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PostmanItem {
    #[serde(default)]
    name: Option<String>,
    /// Present when this item is a folder (group of sub-items).
    #[serde(default)]
    item: Option<Vec<PostmanItem>>,
    /// Present when this item is a request.
    #[serde(default)]
    request: Option<PostmanRequest>,
}

#[derive(Debug, Deserialize)]
struct PostmanRequest {
    #[serde(default)]
    method: Option<String>,
    #[serde(default)]
    header: Vec<PostmanHeader>,
    #[serde(default)]
    url: Option<PostmanUrl>,
    #[serde(default)]
    body: Option<PostmanBody>,
    #[serde(default)]
    auth: Option<PostmanAuth>,
}

#[derive(Debug, Deserialize)]
struct PostmanHeader {
    #[serde(default)]
    key: String,
    #[serde(default)]
    value: String,
    #[serde(default)]
    disabled: bool,
}

/// URL can be either a bare string or an object with `raw`/`query`.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum PostmanUrl {
    Raw(String),
    Object {
        #[serde(default)]
        raw: String,
        #[serde(default)]
        query: Vec<PostmanQuery>,
    },
}

#[derive(Debug, Deserialize)]
struct PostmanQuery {
    #[serde(default)]
    key: String,
    #[serde(default)]
    value: Option<String>,
    #[serde(default)]
    disabled: bool,
}

#[derive(Debug, Deserialize)]
struct PostmanBody {
    #[serde(default)]
    mode: Option<String>,
    #[serde(default)]
    raw: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PostmanAuth {
    #[serde(rename = "type", default)]
    auth_type: Option<String>,
    #[serde(default)]
    bearer: Vec<PostmanAuthParam>,
    #[serde(default)]
    basic: Vec<PostmanAuthParam>,
    #[serde(default)]
    apikey: Vec<PostmanAuthParam>,
}

#[derive(Debug, Deserialize)]
struct PostmanAuthParam {
    #[serde(default)]
    key: String,
    /// Auth param values are usually strings, but the schema allows any JSON.
    #[serde(default)]
    value: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct PostmanVariable {
    #[serde(default)]
    key: Option<String>,
    #[serde(default)]
    value: Option<String>,
}

/// Parse a Postman v2.x collection export into a restui `Collection`.
///
/// `fallback_name` is used when the export omits `info.name` (e.g. the file stem).
pub fn import_postman(json: &str, fallback_name: &str) -> Result<ImportResult> {
    let parsed: PostmanCollection = serde_json::from_str(json)?;

    // Reject the legacy v1 schema, which has a completely different shape.
    if let Some(schema) = &parsed.info.schema {
        if schema.contains("v1.0") {
            bail!("Only Postman v2.x collections are supported (got v1.0)");
        }
    }

    let name = parsed
        .info
        .name
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| fallback_name.to_string());

    let mut collection = Collection::new(name.clone());
    collection.items = convert_items(&parsed.item);
    let request_count = count_requests(&collection.items);

    let environment = build_environment(&name, &parsed.variable);

    Ok(ImportResult {
        collection,
        environment,
        request_count,
    })
}

/// Recursively convert Postman items into restui collection items.
fn convert_items(items: &[PostmanItem]) -> Vec<CollectionItem> {
    items
        .iter()
        .filter_map(|item| {
            let name = item.name.clone().unwrap_or_default();
            if let Some(children) = &item.item {
                // Folder
                Some(CollectionItem::Folder {
                    id: uuid::Uuid::new_v4().to_string(),
                    name,
                    items: convert_items(children),
                    expanded: true,
                })
            } else {
                item.request
                    .as_ref()
                    .map(|req| CollectionItem::Request(convert_request(&name, req)))
            }
        })
        .collect()
}

fn convert_request(name: &str, req: &PostmanRequest) -> ApiRequest {
    let mut request = ApiRequest::new(name);

    request.method = parse_method(req.method.as_deref());

    let (url, query_params) = parse_url(req.url.as_ref());
    request.url = url;
    request.query_params = query_params;

    request.headers = req
        .header
        .iter()
        .map(|h| KeyValue {
            key: h.key.clone(),
            value: h.value.clone(),
            enabled: !h.disabled,
        })
        .collect();

    if let Some(body) = &req.body {
        if body.mode.as_deref() == Some("raw") {
            request.body = body.raw.clone().unwrap_or_default();
        }
    }

    if let Some(auth) = &req.auth {
        request.auth = convert_auth(auth, request.auth);
    }

    request
}

fn parse_method(method: Option<&str>) -> HttpMethod {
    match method.unwrap_or("GET").to_uppercase().as_str() {
        "POST" => HttpMethod::Post,
        "PUT" => HttpMethod::Put,
        "PATCH" => HttpMethod::Patch,
        "DELETE" => HttpMethod::Delete,
        _ => HttpMethod::Get,
    }
}

/// Split the URL into a base (without query string) and query params.
///
/// Prefers the structured `query` array when present, otherwise parses the
/// query string out of `raw`.
fn parse_url(url: Option<&PostmanUrl>) -> (String, Vec<KeyValue>) {
    let (raw, structured) = match url {
        Some(PostmanUrl::Raw(s)) => (s.clone(), Vec::new()),
        Some(PostmanUrl::Object { raw, query }) => (raw.clone(), query_to_kv(query)),
        None => (String::new(), Vec::new()),
    };

    let base = raw.split('?').next().unwrap_or(&raw).to_string();

    if !structured.is_empty() {
        return (base, structured);
    }

    // Fall back to parsing the query string from the raw URL.
    let query_params = raw
        .split_once('?')
        .map(|(_, qs)| {
            qs.split('&')
                .filter(|p| !p.is_empty())
                .map(|pair| {
                    let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
                    KeyValue {
                        key: key.to_string(),
                        value: value.to_string(),
                        enabled: true,
                    }
                })
                .collect()
        })
        .unwrap_or_default();

    (base, query_params)
}

/// Map Postman's structured query array to restui `KeyValue`s.
fn query_to_kv(query: &[PostmanQuery]) -> Vec<KeyValue> {
    query
        .iter()
        .map(|q| KeyValue {
            key: q.key.clone(),
            value: q.value.clone().unwrap_or_default(),
            enabled: !q.disabled,
        })
        .collect()
}

fn convert_auth(auth: &PostmanAuth, mut config: AuthConfig) -> AuthConfig {
    let as_str = |params: &[PostmanAuthParam], key: &str| -> String {
        params
            .iter()
            .find(|p| p.key == key)
            .map(|p| match &p.value {
                serde_json::Value::String(s) => s.clone(),
                other => other.to_string(),
            })
            .unwrap_or_default()
    };

    match auth.auth_type.as_deref() {
        Some("bearer") => {
            config.auth_type = AuthType::Bearer;
            config.bearer_token = as_str(&auth.bearer, "token");
        }
        Some("basic") => {
            config.auth_type = AuthType::Basic;
            config.basic_username = as_str(&auth.basic, "username");
            config.basic_password = as_str(&auth.basic, "password");
        }
        Some("apikey") => {
            config.auth_type = AuthType::ApiKey;
            config.api_key_name = as_str(&auth.apikey, "key");
            config.api_key_value = as_str(&auth.apikey, "value");
            let location = as_str(&auth.apikey, "in");
            config.api_key_location = if location == "query" {
                "query".to_string()
            } else {
                "header".to_string()
            };
        }
        _ => {
            config.auth_type = AuthType::None;
        }
    }

    config
}

fn build_environment(collection_name: &str, variables: &[PostmanVariable]) -> Option<Environment> {
    let pairs: Vec<(String, String)> = variables
        .iter()
        .filter_map(|v| {
            let key = v.key.clone()?;
            if key.trim().is_empty() {
                return None;
            }
            Some((key, v.value.clone().unwrap_or_default()))
        })
        .collect();

    if pairs.is_empty() {
        return None;
    }

    let mut env = Environment::new(format!("{} (imported)", collection_name));
    for (key, value) in pairs {
        env.set(key, value);
    }
    Some(env)
}

fn count_requests(items: &[CollectionItem]) -> usize {
    items
        .iter()
        .map(|item| match item {
            CollectionItem::Request(_) => 1,
            CollectionItem::Folder { items, .. } => count_requests(items),
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{
        "info": {
            "name": "Sample API",
            "schema": "https://schema.getpostman.com/json/collection/v2.1.0/collection.json"
        },
        "item": [
            {
                "name": "Users",
                "item": [
                    {
                        "name": "List Users",
                        "request": {
                            "method": "GET",
                            "header": [
                                { "key": "Accept", "value": "application/json" },
                                { "key": "X-Debug", "value": "1", "disabled": true }
                            ],
                            "url": {
                                "raw": "{{baseUrl}}/users?page=2",
                                "query": [
                                    { "key": "page", "value": "2" },
                                    { "key": "archived", "value": "true", "disabled": true }
                                ]
                            },
                            "auth": {
                                "type": "bearer",
                                "bearer": [ { "key": "token", "value": "{{authToken}}" } ]
                            }
                        }
                    }
                ]
            },
            {
                "name": "Create User",
                "request": {
                    "method": "POST",
                    "header": [],
                    "url": "{{baseUrl}}/users",
                    "body": { "mode": "raw", "raw": "{\"name\":\"Ben\"}" },
                    "auth": {
                        "type": "apikey",
                        "apikey": [
                            { "key": "key", "value": "X-Api-Key" },
                            { "key": "value", "value": "secret" },
                            { "key": "in", "value": "header" }
                        ]
                    }
                }
            }
        ],
        "variable": [
            { "key": "baseUrl", "value": "https://api.example.com" },
            { "key": "authToken", "value": "abc123" }
        ]
    }"#;

    #[test]
    fn test_nested_folder_and_counts() {
        let result = import_postman(SAMPLE, "fallback").unwrap();
        assert_eq!(result.collection.name, "Sample API");
        assert_eq!(result.request_count, 2);

        // Top level: one folder ("Users") and one request ("Create User").
        assert_eq!(result.collection.items.len(), 2);
        match &result.collection.items[0] {
            CollectionItem::Folder { name, items, .. } => {
                assert_eq!(name, "Users");
                assert_eq!(items.len(), 1);
                assert!(matches!(items[0], CollectionItem::Request(_)));
            }
            _ => panic!("expected folder"),
        }
    }

    #[test]
    fn test_method_headers_and_query() {
        let result = import_postman(SAMPLE, "fallback").unwrap();
        let CollectionItem::Folder { items, .. } = &result.collection.items[0] else {
            panic!("expected folder");
        };
        let CollectionItem::Request(req) = &items[0] else {
            panic!("expected request");
        };

        assert_eq!(req.method, HttpMethod::Get);
        assert_eq!(req.url, "{{baseUrl}}/users");

        // Headers: disabled -> enabled false.
        assert_eq!(req.headers.len(), 2);
        assert!(req.headers[0].enabled);
        assert_eq!(req.headers[0].key, "Accept");
        assert!(!req.headers[1].enabled);

        // Query: structured array, disabled -> enabled false.
        assert_eq!(req.query_params.len(), 2);
        assert_eq!(req.query_params[0].key, "page");
        assert!(req.query_params[0].enabled);
        assert!(!req.query_params[1].enabled);
    }

    #[test]
    fn test_bearer_auth() {
        let result = import_postman(SAMPLE, "fallback").unwrap();
        let CollectionItem::Folder { items, .. } = &result.collection.items[0] else {
            panic!("expected folder");
        };
        let CollectionItem::Request(req) = &items[0] else {
            panic!("expected request");
        };
        assert_eq!(req.auth.auth_type, AuthType::Bearer);
        assert_eq!(req.auth.bearer_token, "{{authToken}}");
    }

    #[test]
    fn test_raw_body_and_apikey_auth() {
        let result = import_postman(SAMPLE, "fallback").unwrap();
        let CollectionItem::Request(req) = &result.collection.items[1] else {
            panic!("expected request");
        };
        assert_eq!(req.method, HttpMethod::Post);
        assert_eq!(req.url, "{{baseUrl}}/users");
        assert_eq!(req.body, "{\"name\":\"Ben\"}");
        assert_eq!(req.auth.auth_type, AuthType::ApiKey);
        assert_eq!(req.auth.api_key_name, "X-Api-Key");
        assert_eq!(req.auth.api_key_value, "secret");
        assert_eq!(req.auth.api_key_location, "header");
    }

    #[test]
    fn test_variables_to_environment() {
        let result = import_postman(SAMPLE, "fallback").unwrap();
        let env = result.environment.expect("expected an environment");
        assert_eq!(env.name, "Sample API (imported)");
        assert_eq!(env.get("baseUrl").unwrap(), "https://api.example.com");
        assert_eq!(env.get("authToken").unwrap(), "abc123");
    }

    #[test]
    fn test_fallback_name_when_info_missing() {
        let json = r#"{ "item": [] }"#;
        let result = import_postman(json, "my-export").unwrap();
        assert_eq!(result.collection.name, "my-export");
        assert_eq!(result.request_count, 0);
        assert!(result.environment.is_none());
    }

    #[test]
    fn test_v1_schema_rejected() {
        let json = r#"{ "info": { "name": "Old", "schema": "https://schema.getpostman.com/json/collection/v1.0.0/collection.json" }, "item": [] }"#;
        assert!(import_postman(json, "fallback").is_err());
    }

    #[test]
    fn test_malformed_json_errors() {
        assert!(import_postman("not json", "fallback").is_err());
    }
}
