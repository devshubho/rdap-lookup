//! The RDAP server directory: IANA's bootstrap plus a supplemental overlay.
//!
//! IANA publishes canonical RDAP routing in the [RFC 7484] bootstrap file, but
//! it lists only some ccTLDs (each ccTLD is self-governed). This module parses
//! that file into a `tld -> servers` index and merges a small, hand-maintained
//! overlay that fills the ccTLD gaps. IANA always wins: an overlay entry is only
//! applied when IANA does not already cover the TLD, so the overlay stays a
//! delta and never goes stale against the canonical source.
//!
//! [RFC 7484]: https://datatracker.ietf.org/doc/html/rfc7484

use std::collections::HashMap;

use serde::Deserialize;
use url::Url;

use crate::error::Result;

/// The canonical IANA RDAP bootstrap URL.
pub const IANA_BOOTSTRAP_URL: &str = "https://data.iana.org/rdap/dns.json";

/// The IANA bootstrap document, per RFC 7484: `services` is a list of
/// `[tlds, servers]` pairs.
#[derive(Debug, Clone, Deserialize)]
struct IanaBootstrap {
    #[serde(default)]
    publication: Option<String>,
    #[serde(default)]
    services: Vec<Vec<Vec<String>>>,
}

/// One supplemental ccTLD server record.
#[derive(Debug, Clone, Deserialize)]
pub struct OverlayEntry {
    /// TLD without a leading dot, e.g. `ch`.
    pub tld: String,
    /// Base RDAP URL, e.g. `https://rdap.nic.ch/`.
    pub rdap_server: String,
    /// Human-readable operator, e.g. `SWITCH`.
    #[serde(default)]
    pub backend_operator: Option<String>,
    /// Date the entry was last confirmed, `YYYY-MM-DD`.
    #[serde(default)]
    pub date_updated: Option<String>,
    /// Where the URL came from.
    #[serde(default)]
    pub source: Option<String>,
}

/// A supplemental overlay document.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Overlay {
    #[serde(default)]
    pub servers: Vec<OverlayEntry>,
}

/// A resolved `tld -> RDAP servers` index.
#[derive(Debug, Clone, Default)]
pub struct ServerDirectory {
    map: HashMap<String, Vec<Url>>,
    publication: Option<String>,
}

impl ServerDirectory {
    /// Parse IANA's bootstrap JSON into a directory.
    pub fn from_iana(json: &str) -> Result<Self> {
        let parsed: IanaBootstrap = serde_json::from_str(json)?;
        let mut directory = Self {
            map: HashMap::new(),
            publication: parsed.publication,
        };

        for service in parsed.services {
            let [tlds, servers] = match <[Vec<String>; 2]>::try_from(service) {
                Ok(pair) => pair,
                Err(_) => continue,
            };
            let servers: Vec<Url> = servers.iter().filter_map(|s| Url::parse(s).ok()).collect();
            if servers.is_empty() {
                continue;
            }
            for tld in tlds {
                let normalized = normalize_tld(&tld);
                if normalized.is_empty() {
                    continue;
                }
                let slot = directory.map.entry(normalized).or_default();
                for server in &servers {
                    if !slot.contains(server) {
                        slot.push(server.clone());
                    }
                }
            }
        }

        Ok(directory)
    }

    /// Merge a supplemental overlay, adding only TLDs IANA does not cover.
    ///
    /// Returns the number of TLDs added.
    pub fn merge_overlay(&mut self, overlay: &Overlay) -> usize {
        let mut added = 0;
        for entry in &overlay.servers {
            let tld = normalize_tld(&entry.tld);
            if tld.is_empty() || self.map.contains_key(&tld) {
                continue;
            }
            if let Ok(url) = Url::parse(&entry.rdap_server) {
                self.map.insert(tld, vec![url]);
                added += 1;
            }
        }
        added
    }

    /// The servers known for `tld` (no leading dot), if any.
    pub fn servers_for(&self, tld: &str) -> Option<&[Url]> {
        self.map.get(&normalize_tld(tld)).map(Vec::as_slice)
    }

    /// Whether `tld` is covered by this directory.
    pub fn contains(&self, tld: &str) -> bool {
        self.map.contains_key(&normalize_tld(tld))
    }

    /// The IANA publication timestamp, when present.
    pub fn publication(&self) -> Option<&str> {
        self.publication.as_deref()
    }

    /// Number of TLDs in the directory.
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Whether the directory has no TLDs.
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

/// Lowercase a TLD and strip a leading dot, mirroring IANA's `.com` style.
fn normalize_tld(tld: &str) -> String {
    tld.trim().trim_start_matches('.').to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    const IANA: &str = r#"{
        "description": "test",
        "publication": "2026-01-01T00:00:00Z",
        "services": [
            [["com", "net"], ["https://rdap.example/gtld"]],
            [["ch"], ["https://rdap.iana.ch/"]]
        ]
    }"#;

    #[test]
    fn parses_iana_services() {
        let directory = ServerDirectory::from_iana(IANA).unwrap();
        assert_eq!(directory.len(), 3);
        assert_eq!(
            directory.servers_for("com").unwrap()[0].as_str(),
            "https://rdap.example/gtld"
        );
        assert_eq!(directory.publication(), Some("2026-01-01T00:00:00Z"));
    }

    #[test]
    fn overlay_fills_gaps_only() {
        let mut directory = ServerDirectory::from_iana(IANA).unwrap();
        let overlay = Overlay {
            servers: vec![
                OverlayEntry {
                    tld: "io".into(),
                    rdap_server: "https://rdap.nic.io/".into(),
                    backend_operator: None,
                    date_updated: None,
                    source: None,
                },
                OverlayEntry {
                    tld: "ch".into(),
                    rdap_server: "https://evil.example/".into(),
                    backend_operator: None,
                    date_updated: None,
                    source: None,
                },
            ],
        };
        let added = directory.merge_overlay(&overlay);
        assert_eq!(added, 1, "only the uncovered TLD is added");
        assert_eq!(
            directory.servers_for("io").unwrap()[0].as_str(),
            "https://rdap.nic.io/"
        );
        assert_eq!(
            directory.servers_for("ch").unwrap()[0].as_str(),
            "https://rdap.iana.ch/",
            "IANA wins over the overlay"
        );
    }

    #[test]
    fn handles_leading_dot_and_case() {
        let directory = ServerDirectory::from_iana(IANA).unwrap();
        assert!(directory.contains(".COM"));
        assert_eq!(
            directory.servers_for("NET").unwrap()[0].as_str(),
            "https://rdap.example/gtld"
        );
    }
}
