//! A tiny HTTP cache that respects `ETag` / `Last-Modified`.
//!
//! External tools should not re-download the bootstrap on every run, and we
//! should be a good citizen to the origin. Bodies are written to disk keyed on
//! the URL, alongside a metadata sidecar holding the response validators.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

#[derive(Debug, Default, Serialize, Deserialize)]
struct CacheMeta {
    url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    etag: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    last_modified: Option<String>,
}

/// An on-disk, validator-aware HTTP cache.
#[derive(Debug, Clone)]
pub struct Cache {
    dir: PathBuf,
    client: reqwest::Client,
}

impl Cache {
    /// Create a cache rooted at `dir`. The directory is created lazily.
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self {
            dir: dir.into(),
            client: reqwest::Client::new(),
        }
    }

    /// The underlying HTTP client, for callers that need to make their own requests.
    pub fn client(&self) -> &reqwest::Client {
        &self.client
    }

    /// The directory backing this cache.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn key(url: &str) -> String {
        let mut hasher = DefaultHasher::new();
        url.hash(&mut hasher);
        format!("{:016x}", hasher.finish())
    }

    fn paths(&self, url: &str) -> (PathBuf, PathBuf) {
        let key = Self::key(url);
        (
            self.dir.join(format!("{key}.body")),
            self.dir.join(format!("{key}.meta.json")),
        )
    }

    /// Return the cached body for `url`, revalidating with the origin first.
    ///
    /// Sends `If-None-Match` / `If-Modified-Since` when validators are known, so
    /// an unchanged source costs a `304` rather than a full download.
    pub async fn get_or_fetch(&self, url: &str) -> Result<String> {
        std::fs::create_dir_all(&self.dir)?;
        let (body_path, meta_path) = self.paths(url);

        let meta: CacheMeta = std::fs::read_to_string(&meta_path)
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default();

        let mut request = self.client.get(url).header("accept", "application/json");
        if let Some(etag) = &meta.etag {
            request = request.header("if-none-match", etag);
        }
        if let Some(last_modified) = &meta.last_modified {
            request = request.header("if-modified-since", last_modified);
        }

        let response = request.send().await?;

        if response.status() == reqwest::StatusCode::NOT_MODIFIED {
            return std::fs::read_to_string(&body_path).map_err(Error::from);
        }

        let response = response.error_for_status()?;
        let new_meta = CacheMeta {
            url: url.to_string(),
            etag: header(&response, "etag"),
            last_modified: header(&response, "last-modified"),
        };
        let body = response.text().await?;

        std::fs::write(&body_path, &body)?;
        std::fs::write(&meta_path, serde_json::to_string_pretty(&new_meta)?)?;

        Ok(body)
    }
}

fn header(response: &reqwest::Response, name: &str) -> Option<String> {
    response
        .headers()
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string)
}
