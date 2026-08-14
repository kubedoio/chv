use reqwest::Client;
use serde_json::Value;
use std::fmt;

#[derive(Debug)]
pub enum CliError {
    Http(String),
    Api { status: u16, message: String },
    Parse(String),
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CliError::Http(msg) => write!(f, "HTTP error: {msg}"),
            CliError::Api { status, message } => {
                write!(f, "API error (HTTP {status}): {message}")
            }
            CliError::Parse(msg) => write!(f, "Parse error: {msg}"),
        }
    }
}

impl std::error::Error for CliError {}

pub struct BffClient {
    base_url: String,
    token: Option<String>,
    http: Client,
}

impl BffClient {
    pub fn new(base_url: String, token: Option<String>) -> Self {
        let http = Client::builder()
            .danger_accept_invalid_certs(true)
            .build()
            .expect("failed to build HTTP client");

        Self {
            base_url,
            token,
            http,
        }
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }

    fn apply_auth(&self, builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        if let Some(ref token) = self.token {
            builder.header("Authorization", format!("Bearer {token}"))
        } else {
            builder
        }
    }

    async fn handle_response(&self, resp: reqwest::Response) -> Result<Value, CliError> {
        let status = resp.status().as_u16();
        let body = resp
            .text()
            .await
            .map_err(|e| CliError::Http(e.to_string()))?;

        if status >= 400 {
            let message = serde_json::from_str::<Value>(&body)
                .ok()
                .and_then(|v| v.get("error").and_then(|e| e.as_str()).map(String::from))
                .unwrap_or(body);
            return Err(CliError::Api { status, message });
        }

        if body.is_empty() {
            return Ok(Value::Null);
        }

        serde_json::from_str(&body).map_err(|e| CliError::Parse(e.to_string()))
    }

    pub async fn get(&self, path: &str) -> Result<Value, CliError> {
        let req = self.http.get(self.url(path));
        let req = self.apply_auth(req);
        let resp = req
            .send()
            .await
            .map_err(|e| CliError::Http(e.to_string()))?;
        self.handle_response(resp).await
    }

    pub async fn post(&self, path: &str, body: &Value) -> Result<Value, CliError> {
        let req = self.http.post(self.url(path)).json(body);
        let req = self.apply_auth(req);
        let resp = req
            .send()
            .await
            .map_err(|e| CliError::Http(e.to_string()))?;
        self.handle_response(resp).await
    }

    pub async fn delete(&self, path: &str) -> Result<Value, CliError> {
        let req = self.http.delete(self.url(path));
        let req = self.apply_auth(req);
        let resp = req
            .send()
            .await
            .map_err(|e| CliError::Http(e.to_string()))?;
        self.handle_response(resp).await
    }
}
