//! `rdap-lookup` resolves a domain to its RDAP server and queries it.
//!
//! The routing table is IANA's RFC 7484 bootstrap merged with a small,
//! hand-maintained overlay of ccTLD servers that IANA omits. IANA always wins,
//! so the overlay stays a delta. The bootstrap is cached on disk and
//! revalidated with `ETag` / `Last-Modified`.
//!
//! ```no_run
//! use rdap_lookup::{Cache, IANA_BOOTSTRAP_URL, Overlay, RdapClient, ServerDirectory};
//!
//! # async fn run() -> anyhow::Result<()> {
//! let cache = Cache::new(std::env::temp_dir().join("rdap-lookup"));
//! let iana = cache.get_or_fetch(IANA_BOOTSTRAP_URL).await?;
//! let mut directory = ServerDirectory::from_iana(&iana)?;
//!
//! let overlay: Overlay = serde_json::from_str(include_str!("../data/rdap-servers.json"))?;
//! directory.merge_overlay(&overlay);
//!
//! let tld = rdap_lookup::resolve_tld("example.com")?;
//! let domain = rdap_lookup::to_ascii("example.com")?;
//! if let Some(server) = directory.servers_for(&tld).and_then(|s| s.first()) {
//!     let record = RdapClient::new().lookup(server, &domain).await?;
//!     println!("{record}");
//! }
//! # Ok(())
//! # }
//! ```

pub mod bootstrap;
pub mod cache;
pub mod classify;
pub mod client;
pub mod error;
pub mod resolver;

pub use bootstrap::{IANA_BOOTSTRAP_URL, Overlay, OverlayEntry, ServerDirectory};
pub use cache::Cache;
pub use classify::{Kind, TldIndex};
pub use client::RdapClient;
pub use error::{Error, Result};
pub use resolver::{is_idn, resolve_tld, to_ascii, to_unicode};

/// The current time in UTC, formatted as `YYYY-MM-DDTHH:MM:SSZ`.
///
/// The trailing `Z` marks UTC, matching IANA's bootstrap publication format.
///
/// ```
/// let stamp = rdap_lookup::utc_now_z();
/// assert!(stamp.ends_with('Z'));
/// assert_eq!(stamp.len(), 20);
/// ```
pub fn utc_now_z() -> String {
    let now = time::OffsetDateTime::now_utc();
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        now.year(),
        u8::from(now.month()),
        now.day(),
        now.hour(),
        now.minute(),
        now.second(),
    )
}
