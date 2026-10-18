//! Integration tests over the shipped overlay and the resolve/merge pipeline.

use rdap_lookup::{Overlay, ServerDirectory, TldIndex, resolve_tld, to_ascii};

const OVERLAY: &str = include_str!("../data/manual/cctld-rdap-servers.json");
const PUNYCODE_OVERLAY: &str = include_str!("../data/manual/punycode-rdap-servers.json");

const IANA: &str = r#"{
    "publication": "2026-01-01T00:00:00Z",
    "services": [
        [["com", "net"], ["https://rdap.example/gtld"]],
        [["ch"], ["https://rdap.iana.ch/"]]
    ]
}"#;

#[test]
fn shipped_overlay_parses_and_is_nonempty() {
    let overlay: Overlay = serde_json::from_str(OVERLAY).expect("overlay parses");
    assert!(!overlay.servers.is_empty());
    for entry in &overlay.servers {
        assert!(!entry.tld.starts_with('.'), "tld has no leading dot");
        assert!(
            entry.rdap_server.starts_with("https://"),
            "{} is https",
            entry.tld
        );
    }
}

#[test]
fn overlay_is_merged_over_iana_bootstrap() {
    let mut directory = ServerDirectory::from_iana(IANA).unwrap();
    let overlay: Overlay = serde_json::from_str(OVERLAY).unwrap();
    let added = directory.merge_overlay(&overlay);

    assert_eq!(
        added,
        overlay.servers.len() - 1,
        "every overlay TLD is added except `ch`, which the stub already covers"
    );
    assert!(directory.contains("ch"));
    assert_eq!(
        directory.servers_for("ch").unwrap()[0].as_str(),
        "https://rdap.iana.ch/",
        "IANA wins for a TLD it already covers"
    );
    assert_eq!(
        directory.servers_for("de").unwrap()[0].as_str(),
        "https://rdap.denic.de/"
    );
}

#[test]
fn resolves_cc_tld_from_overlay() {
    let mut directory = ServerDirectory::from_iana(IANA).unwrap();
    let overlay: Overlay = serde_json::from_str(OVERLAY).unwrap();
    directory.merge_overlay(&overlay);

    let tld = resolve_tld("nic.io").unwrap();
    assert_eq!(tld, "io");
    assert!(directory.servers_for(&tld).is_some());
}

#[test]
fn idn_domains_are_punycoded() {
    assert_eq!(to_ascii("пример.рф").unwrap(), "xn--e1afmkfd.xn--p1ai");
}

#[test]
fn published_overlay_matches_source() {
    const PUBLISHED: &str = include_str!("../site/rdap-cctld-servers.json");
    assert_eq!(
        PUBLISHED, OVERLAY,
        "site/rdap-cctld-servers.json must stay byte-identical to data/manual/cctld-rdap-servers.json"
    );
}

#[test]
fn published_punycode_overlay_matches_source() {
    const PUBLISHED: &str = include_str!("../site/punycode-rdap-servers.json");
    assert_eq!(
        PUBLISHED, PUNYCODE_OVERLAY,
        "site/punycode-rdap-servers.json must stay byte-identical to data/manual/punycode-rdap-servers.json"
    );
}

#[test]
fn punycode_overlay_holds_only_alabels() {
    let overlay: Overlay = serde_json::from_str(PUNYCODE_OVERLAY).expect("punycode overlay parses");
    for entry in &overlay.servers {
        assert!(
            entry.tld.starts_with("xn--"),
            "punycode overlay entry must be an A-label, got {}",
            entry.tld
        );
    }
}

#[test]
fn published_bootstrap_matches_generated() {
    const GENERATED: &str = include_str!("../data/generated/dns.json");
    const PUBLISHED: &str = include_str!("../site/dns.json");
    assert_eq!(
        PUBLISHED, GENERATED,
        "site/dns.json must stay byte-identical to data/generated/dns.json"
    );
}

#[test]
fn published_bootstrap_includes_overlay() {
    const PUBLISHED: &str = include_str!("../site/dns.json");
    let directory = ServerDirectory::from_iana(PUBLISHED).expect("site/dns.json parses");
    for source in [OVERLAY, PUNYCODE_OVERLAY] {
        let overlay: Overlay = serde_json::from_str(source).unwrap();
        for entry in &overlay.servers {
            assert!(
                directory.contains(&entry.tld),
                "site/dns.json should route .{}",
                entry.tld
            );
        }
    }
}

#[test]
fn overlay_entries_are_delegated_cctlds() {
    let index = TldIndex::embedded();
    let overlay: Overlay = serde_json::from_str(OVERLAY).unwrap();
    for entry in &overlay.servers {
        assert!(index.is_cctld(&entry.tld), "{} is a ccTLD", entry.tld);
        assert!(index.is_delegated(&entry.tld), "{} is delegated", entry.tld);
    }
}

#[test]
fn idn_helpers_round_trip() {
    assert!(rdap_lookup::is_idn("xn--p1ai"));
    assert!(!rdap_lookup::is_idn("com"));
    assert_eq!(rdap_lookup::to_unicode("xn--p1ai").as_deref(), Some("рф"));
    assert_eq!(rdap_lookup::to_unicode("com"), None);
}

#[test]
fn overlay_carries_a_publication_timestamp() {
    let overlay: Overlay = serde_json::from_str(OVERLAY).unwrap();
    let publication = overlay.publication().expect("publication present");
    assert!(
        publication.ends_with('Z') && publication.len() == 20,
        "expected ISO 8601 UTC like 2026-10-18T07:24:09Z, got {publication}"
    );
}
