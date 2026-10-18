//! TLD classification: which TLDs are ccTLDs, and which are delegated.
//!
//! IANA's RDAP bootstrap lists every gTLD but only *some* ccTLDs, and it never
//! says which kind a TLD is. This module reads a small, generated index
//! (`data/tlds.json`, derived from IANA's Root DB and the root-zone TLD list) so
//! the project can label overlay entries and report ccTLD coverage without
//! guessing from string length — which would misclassify IDN ccTLDs.

use std::collections::HashMap;

use serde::Deserialize;

use crate::error::Result;

/// The IANA classification of a TLD.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A country-code TLD (IANA `country-code`).
    Cctld,
    /// A generic TLD (`generic`, `sponsored`, or `generic-restricted`).
    Gtld,
    /// The infrastructure TLD (`arpa`).
    Infrastructure,
}

impl Kind {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "cctld" => Some(Self::Cctld),
            "gtld" => Some(Self::Gtld),
            "infrastructure" => Some(Self::Infrastructure),
            _ => None,
        }
    }
}

#[derive(Debug, Deserialize)]
struct RawEntry {
    tld: String,
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    delegated: bool,
}

#[derive(Debug, Deserialize)]
struct RawIndex {
    #[serde(default)]
    publication: Option<String>,
    tlds: Vec<RawEntry>,
}

#[derive(Debug, Clone, Copy)]
struct Entry {
    kind: Kind,
    delegated: bool,
}

/// A `tld -> classification` index.
#[derive(Debug, Clone, Default)]
pub struct TldIndex {
    map: HashMap<String, Entry>,
    publication: Option<String>,
}

impl TldIndex {
    /// Load the index embedded in the binary (`data/tlds.json`).
    ///
    /// # Panics
    ///
    /// Panics if the embedded index is malformed; that is a build-time bug.
    pub fn embedded() -> Self {
        Self::from_json(include_str!("../data/tlds.json")).expect("embedded data/tlds.json parses")
    }

    /// Parse an index from JSON.
    pub fn from_json(json: &str) -> Result<Self> {
        let parsed: RawIndex = serde_json::from_str(json)?;
        let mut map = HashMap::with_capacity(parsed.tlds.len());
        for entry in parsed.tlds {
            if let Some(kind) = Kind::parse(&entry.kind) {
                let info = Entry {
                    kind,
                    delegated: entry.delegated,
                };
                map.insert(normalize(&entry.tld), info);
            }
        }
        Ok(Self {
            map,
            publication: parsed.publication,
        })
    }

    /// The index's publication timestamp, when present.
    pub fn publication(&self) -> Option<&str> {
        self.publication.as_deref()
    }

    /// The classification of `tld`, if known.
    pub fn kind(&self, tld: &str) -> Option<Kind> {
        self.map.get(&normalize(tld)).map(|entry| entry.kind)
    }

    /// Whether `tld` is delegated (present in the DNS root zone).
    pub fn is_delegated(&self, tld: &str) -> bool {
        self.map.get(&normalize(tld)).is_some_and(|e| e.delegated)
    }

    /// Whether `tld` is a country-code TLD.
    pub fn is_cctld(&self, tld: &str) -> bool {
        self.kind(tld) == Some(Kind::Cctld)
    }

    /// Every TLD in the index, sorted.
    pub fn tlds(&self) -> Vec<String> {
        let mut tlds: Vec<String> = self.map.keys().cloned().collect();
        tlds.sort();
        tlds
    }

    /// Number of TLDs in the index.
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Whether the index has no TLDs.
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

/// Lowercase a TLD and strip a leading dot, matching IANA's style.
fn normalize(tld: &str) -> String {
    tld.trim().trim_start_matches('.').to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_index_classifies_kinds() {
        let index = TldIndex::embedded();
        assert!(!index.is_empty());
        assert_eq!(index.kind("com"), Some(Kind::Gtld));
        assert_eq!(index.kind(".CH"), Some(Kind::Cctld));
        assert_eq!(index.kind("xn--p1ai"), Some(Kind::Cctld), "IDN ccTLD");
        assert_eq!(index.kind("xn--fiqs8s"), Some(Kind::Cctld), "IDN ccTLD");
        assert_eq!(index.kind("arpa"), Some(Kind::Infrastructure));
    }

    #[test]
    fn unknown_tlds_are_not_classified() {
        let index = TldIndex::embedded();
        assert_eq!(index.kind("definitely-not-a-tld"), None);
        assert!(!index.is_delegated("definitely-not-a-tld"));
        assert!(!index.is_cctld("definitely-not-a-tld"));
    }

    #[test]
    fn parses_a_minimal_index() {
        let index = TldIndex::from_json(
            r#"{"tlds":[{"tld":"ch","type":"cctld","delegated":true},
                       {"tld":"zz","type":"gtld","delegated":false}]}"#,
        )
        .unwrap();
        assert!(index.is_cctld("ch"));
        assert!(index.is_delegated("ch"));
        assert!(!index.is_delegated("zz"));
    }
}
