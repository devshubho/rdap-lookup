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

use std::collections::{BTreeMap, HashMap};

use serde::Deserialize;
use serde_json::{Value, json};
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
    /// When the overlay was last published, ISO 8601 with a `Z` suffix, e.g.
    /// `2026-10-18T07:24:09Z`.
    #[serde(default)]
    pub publication: Option<String>,
    #[serde(default)]
    pub servers: Vec<OverlayEntry>,
}

impl Overlay {
    /// The overlay's publication timestamp, when present.
    pub fn publication(&self) -> Option<&str> {
        self.publication.as_deref()
    }

    /// Set `publication` to the current UTC time (`YYYY-MM-DDTHH:MM:SSZ`).
    pub fn stamp(&mut self) {
        self.publication = Some(crate::utc_now_z());
    }
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

    /// Render the merged directory as an RFC 7484 bootstrap document.
    ///
    /// The output uses IANA's own schema (`services` of `[tlds, servers]`
    /// pairs), so the merged directory can be served at a single stable URL and
    /// consumed by any RDAP client as a drop-in bootstrap replacement. TLDs
    /// that share an identical server list are grouped together.
    ///
    /// `publication` overrides the timestamp; when `None`, the directory's IANA
    /// publication timestamp is reused.
    pub fn to_bootstrap_json(&self, publication: Option<&str>) -> Result<String> {
        let mut groups: BTreeMap<Vec<String>, Vec<String>> = BTreeMap::new();
        for (tld, servers) in &self.map {
            let key: Vec<String> = servers.iter().map(|s| s.as_str().to_owned()).collect();
            groups.entry(key).or_default().push(tld.clone());
        }

        let mut services: Vec<Value> = Vec::new();
        for (servers, mut tlds) in groups {
            tlds.sort();
            services.push(json!([tlds, servers]));
        }

        let mut document = json!({
            "version": "1.0",
            "description": "RDAP bootstrap for the Domain Name System. Independently \
                published by rdapserver.org: IANA's canonical RFC 7484 bootstrap \
                merged with supplemental ccTLD RDAP servers for TLDs IANA omits. \
                IANA entries always take precedence. Not affiliated with or \
                endorsed by IANA.",
            "services": services,
        });
        if let Some(publication) = publication.or(self.publication.as_deref()) {
            document["publication"] = json!(publication);
        }

        Ok(serde_json::to_string_pretty(&document)?)
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
            publication: None,
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
    fn stamp_sets_utc_publication() {
        let mut overlay = Overlay::default();
        overlay.stamp();
        let stamp = overlay.publication().expect("stamped");
        assert!(stamp.ends_with('Z'), "got {stamp}");
        assert_eq!(stamp.len(), 20, "got {stamp}");
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

    #[test]
    fn renders_merged_rfc7484_bootstrap() {
        let mut directory = ServerDirectory::from_iana(IANA).unwrap();
        let overlay = Overlay {
            publication: Some("2026-02-02T00:00:00Z".into()),
            servers: vec![OverlayEntry {
                tld: "io".into(),
                rdap_server: "https://rdap.nic.io/".into(),
                backend_operator: None,
                date_updated: None,
                source: None,
            }],
        };
        directory.merge_overlay(&overlay);

        let json = directory.to_bootstrap_json(overlay.publication()).unwrap();
        let value: Value = serde_json::from_str(&json).unwrap();

        assert_eq!(value["publication"], "2026-02-02T00:00:00Z");
        let services = value["services"].as_array().unwrap();
        let has = |tld: &str, url: &str| {
            services.iter().any(|service| {
                service[0].as_array().unwrap().iter().any(|t| t == tld)
                    && service[1].as_array().unwrap().iter().any(|s| s == url)
            })
        };
        assert!(has("com", "https://rdap.example/gtld"), "IANA entry kept");
        assert!(has("io", "https://rdap.nic.io/"), "overlay entry merged");

        let reparsed = ServerDirectory::from_iana(&json).unwrap();
        assert_eq!(reparsed.len(), directory.len());
        assert_eq!(
            reparsed.servers_for("io").unwrap()[0].as_str(),
            "https://rdap.nic.io/"
        );
    }
}
