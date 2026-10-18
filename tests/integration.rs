//! Integration tests over the shipped overlay and the resolve/merge pipeline.

use rdap_lookup::{Overlay, ServerDirectory, resolve_tld, to_ascii};

const OVERLAY: &str = include_str!("../data/rdap-servers.json");

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
