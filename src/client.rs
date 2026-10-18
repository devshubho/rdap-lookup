//! A minimal RDAP client: query a server for a domain's RDAP record.

use url::Url;

use crate::error::Result;

/// The RDAP media type for a domain lookup (RFC 9083).
pub const RDAP_DOMAIN_MEDIA_TYPE: &str = "application/rdap+json";

/// A thin wrapper over an HTTP client that issues RDAP domain queries.
#[derive(Debug, Clone)]
pub struct RdapClient {
    client: reqwest::Client,
}

impl Default for RdapClient {
    fn default() -> Self {
        Self::new()
    }
}

impl RdapClient {
    /// Create a client with a default HTTP configuration.
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }

    /// Wrap an existing HTTP client.
    pub fn with_client(client: reqwest::Client) -> Self {
        Self { client }
    }

    /// Build the RFC 9082 query URL: `{base}/domain/{name}`.
    ///
    /// ```
    /// # use rdap_lookup::RdapClient;
    /// # use url::Url;
    /// let base = Url::parse("https://rdap.verisign.com/com/v1/").unwrap();
    /// assert_eq!(
    ///     RdapClient::domain_url(&base, "example.com").as_str(),
    ///     "https://rdap.verisign.com/com/v1/domain/example.com",
    /// );
    /// ```
    pub fn domain_url(base: &Url, domain: &str) -> Url {
        let mut url = base.clone();
        let path = format!("{}/domain/{}", url.path().trim_end_matches('/'), domain);
        url.set_path(&path);
        url
    }

    /// Fetch the RDAP domain object for `domain` from `base`.
    pub async fn lookup(&self, base: &Url, domain: &str) -> Result<serde_json::Value> {
        let url = Self::domain_url(base, domain);
        let response = self
            .client
            .get(url)
            .header("accept", RDAP_DOMAIN_MEDIA_TYPE)
            .send()
            .await?
            .error_for_status()?;
        Ok(response.json().await?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_url_with_trailing_slash() {
        let base = Url::parse("https://rdap.verisign.com/com/v1/").unwrap();
        assert_eq!(
            RdapClient::domain_url(&base, "example.com").as_str(),
            "https://rdap.verisign.com/com/v1/domain/example.com"
        );
    }

    #[test]
    fn builds_url_without_trailing_slash() {
        let base = Url::parse("https://rdap.nic.ch").unwrap();
        assert_eq!(
            RdapClient::domain_url(&base, "nic.ch").as_str(),
            "https://rdap.nic.ch/domain/nic.ch"
        );
    }
}
