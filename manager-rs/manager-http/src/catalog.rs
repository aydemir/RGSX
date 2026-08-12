//! Faz 10c/3/2 — katalog route'ları için `CatalogSource` trait + Python proxy.
//!
//! Strangler/atest misali: Rust `manager-http` katalog handler'ları (`platforms`,
//! `search`, `games`, `translations`, `image`), gerçek mantığı Python'da bırakıp
//! onu `127.0.0.1:5000` (veya `RGSX_PYTHON_MANAGER_URL`) üzerinden proxy'ler.
//! Bu, contract'ı birebir korur (Python yanıtı aynen iletilir) ve native Rust
//! portunu (dış ROM kaynak istemcileri) ayrı bir alt faz'a erteletir. `catalog`
//! `None` ise handler'lar mevcut placeholder davranışına düşer (geriye uyumlu).

use async_trait::async_trait;
use serde_json::Value;

/// Katalog kaynağı hatası (proxy çökmesi → handler placeholder'a düşer).
#[derive(Debug)]
pub struct CatalogError(pub String);

/// Katalog veri kaynağı — test'te `FakeCatalog` ile enjekte edilebilir.
#[async_trait]
pub trait CatalogSource: Send + Sync {
    /// JSON dönen GET route'u proxy'ler (ör. `/api/platforms`, `/api/search?q=zelda`).
    async fn get_json(&self, route: &str) -> Result<Value, CatalogError>;
    /// JSON dönen POST route'u proxy'ler (gövde iletilir).
    async fn post_json(&self, route: &str, body: &Value) -> Result<Value, CatalogError>;
    /// İkili (zip) POST route'u proxy'ler (ham bayt + content-type).
    async fn post_binary(&self, route: &str, body: &Value) -> Result<(Vec<u8>, String), CatalogError>;
    /// Box-art görselini (ham bayt + content-type) proxy'ler.
    async fn get_image(&self, platform: &str) -> Result<(Vec<u8>, String), CatalogError>;
}

/// Python `ManagerHandler` (HTTP port `RGSX_PYTHON_MANAGER_URL`) proxy'si.
pub struct PythonCatalog {
    base: String,
    client: reqwest::Client,
}

impl PythonCatalog {
    pub fn new(base: String) -> Self {
        Self {
            base,
            client: reqwest::Client::new(),
        }
    }
}

fn encode(seg: &str) -> String {
    percent_encoding::utf8_percent_encode(seg, percent_encoding::NON_ALPHANUMERIC).to_string()
}

#[async_trait]
impl CatalogSource for PythonCatalog {
    async fn get_json(&self, route: &str) -> Result<Value, CatalogError> {
        let url = format!("{}{}", self.base, route);
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| CatalogError(e.to_string()))?;
        let v: Value = resp
            .json()
            .await
            .map_err(|e| CatalogError(e.to_string()))?;
        Ok(v)
    }

    async fn get_image(&self, platform: &str) -> Result<(Vec<u8>, String), CatalogError> {
        let url = format!("{}/api/image/{}", self.base, encode(platform));
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| CatalogError(e.to_string()))?;
        let ct = resp
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("image/png")
            .to_string();
        let bytes = resp
            .bytes()
            .await
            .map_err(|e| CatalogError(e.to_string()))?;
        Ok((bytes.to_vec(), ct))
    }

    async fn post_json(&self, route: &str, body: &Value) -> Result<Value, CatalogError> {
        let url = format!("{}{}", self.base, route);
        let resp = self
            .client
            .post(&url)
            .json(body)
            .send()
            .await
            .map_err(|e| CatalogError(e.to_string()))?;
        let v: Value = resp
            .json()
            .await
            .map_err(|e| CatalogError(e.to_string()))?;
        Ok(v)
    }

    async fn post_binary(&self, route: &str, body: &Value) -> Result<(Vec<u8>, String), CatalogError> {
        let url = format!("{}{}", self.base, route);
        let resp = self
            .client
            .post(&url)
            .json(body)
            .send()
            .await
            .map_err(|e| CatalogError(e.to_string()))?;
        let ct = resp
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("application/octet-stream")
            .to_string();
        let bytes = resp
            .bytes()
            .await
            .map_err(|e| CatalogError(e.to_string()))?;
        Ok((bytes.to_vec(), ct))
    }
}
