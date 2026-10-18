//! `rdap-lookup` CLI: resolve a domain to its RDAP server and (optionally)
//! fetch its RDAP record.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::Parser;
use rdap_lookup::{
    Cache, IANA_BOOTSTRAP_URL, Overlay, RdapClient, ServerDirectory, TldIndex, resolve_tld,
    to_ascii,
};
use tracing_subscriber::EnvFilter;

/// Resolve a domain to its RDAP server and query it.
#[derive(Debug, Parser)]
#[command(name = "rdap-lookup", version, about, long_about = None)]
struct Cli {
    /// Domain to look up, e.g. `example.com` or `nic.ch`.
    domain: Option<String>,

    /// Directory of supplemental overlay JSON files; every `*.json` is merged
    /// (in filename order). IANA always wins over any overlay entry.
    #[arg(long, default_value = "data/manual")]
    manual_dir: PathBuf,

    /// Directory for the cached IANA bootstrap.
    #[arg(long, env = "RDAP_LOOKUP_CACHE")]
    cache_dir: Option<PathBuf>,

    /// IANA bootstrap URL (override for self-hosting or testing).
    #[arg(long, default_value_t = String::from(IANA_BOOTSTRAP_URL))]
    bootstrap_url: String,

    /// Read the IANA bootstrap from a local file instead of fetching it.
    #[arg(long, value_name = "PATH")]
    iana: Option<PathBuf>,

    /// Print the RDAP server URL without querying it.
    #[arg(long)]
    server_only: bool,

    /// Merge IANA + overlay and write a single RFC 7484 bootstrap JSON to
    /// PATH (use `-` for stdout) instead of looking up a domain.
    #[arg(long, value_name = "PATH")]
    emit_bootstrap: Option<PathBuf>,

    /// TLD classification index JSON (defaults to the embedded copy).
    #[arg(long, value_name = "PATH")]
    tlds: Option<PathBuf>,

    /// Print delegated-ccTLD RDAP coverage and the remaining gaps.
    #[arg(long)]
    coverage: bool,

    /// Emit machine-readable JSON (used with `--coverage`).
    #[arg(long)]
    json: bool,
}

fn default_cache_dir() -> PathBuf {
    std::env::temp_dir().join("rdap-lookup")
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_target(false)
        .init();

    let cli = Cli::parse();

    let cache = Cache::new(cli.cache_dir.clone().unwrap_or_else(default_cache_dir));
    let iana = match cli.iana.as_deref() {
        Some(path) => std::fs::read_to_string(path)
            .with_context(|| format!("reading IANA bootstrap {}", path.display()))?,
        None => cache
            .get_or_fetch(&cli.bootstrap_url)
            .await
            .context("fetching IANA RDAP bootstrap")?,
    };
    let mut directory = ServerDirectory::from_iana(&iana).context("parsing IANA RDAP bootstrap")?;

    let overlays = load_overlays(&cli)?;

    if cli.coverage {
        let index = load_index(cli.tlds.as_deref())?;
        report_coverage(&directory, &overlays, &index, cli.json)?;
        return Ok(());
    }

    let mut added = 0usize;
    let mut overlay_publication: Option<String> = None;
    for overlay in &overlays {
        added += directory.merge_overlay(overlay);
        if let Some(publication) = overlay.publication()
            && overlay_publication
                .as_deref()
                .is_none_or(|current| publication > current)
        {
            overlay_publication = Some(publication.to_owned());
        }
    }
    if added > 0 {
        tracing::info!(
            "merged {added} supplemental server(s) from {} overlay file(s)",
            overlays.len()
        );
    }

    if let Some(path) = cli.emit_bootstrap.as_deref() {
        let document = directory.to_bootstrap_json(overlay_publication.as_deref())?;
        if path == Path::new("-") {
            print!("{document}");
        } else {
            std::fs::write(path, document)
                .with_context(|| format!("writing merged bootstrap to {}", path.display()))?;
            tracing::info!(
                "wrote {len} TLDs to {path}",
                len = directory.len(),
                path = path.display()
            );
        }
        return Ok(());
    }

    let domain_arg = cli
        .domain
        .as_deref()
        .context("a DOMAIN is required unless --emit-bootstrap is used")?;
    let tld = resolve_tld(domain_arg)?;
    let domain = to_ascii(domain_arg)?;
    let server = directory
        .servers_for(&tld)
        .and_then(|servers| servers.first())
        .with_context(|| format!("no RDAP server known for .{tld}"))?;

    println!("{domain}\t.{tld}\t{server}");

    if cli.server_only {
        return Ok(());
    }

    let record = RdapClient::new().lookup(server, &domain).await;
    match record {
        Ok(value) => println!("{}", serde_json::to_string_pretty(&value)?),
        Err(error) => {
            eprintln!("RDAP query failed: {error}");
            std::process::exit(2);
        }
    }

    Ok(())
}

/// Load every `*.json` overlay in the manual directory, sorted by filename so
/// merging is deterministic.
fn load_overlays(cli: &Cli) -> Result<Vec<Overlay>> {
    let dir = &cli.manual_dir;
    if !dir.is_dir() {
        tracing::warn!(
            "overlay directory {} not found; using IANA bootstrap only",
            dir.display()
        );
        return Ok(Vec::new());
    }

    let mut paths: Vec<PathBuf> = std::fs::read_dir(dir)
        .with_context(|| format!("reading overlay directory {}", dir.display()))?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("json"))
        })
        .collect();
    paths.sort();

    let mut overlays = Vec::with_capacity(paths.len());
    for path in paths {
        let raw = std::fs::read_to_string(&path)
            .with_context(|| format!("reading overlay {}", path.display()))?;
        let overlay = serde_json::from_str(&raw)
            .with_context(|| format!("parsing overlay {}", path.display()))?;
        overlays.push(overlay);
    }
    Ok(overlays)
}

/// Load the TLD classification index from `path`, or the embedded copy.
fn load_index(path: Option<&Path>) -> Result<TldIndex> {
    match path {
        Some(path) => {
            let raw = std::fs::read_to_string(path)
                .with_context(|| format!("reading TLD index {}", path.display()))?;
            TldIndex::from_json(&raw)
                .with_context(|| format!("parsing TLD index {}", path.display()))
        }
        None => Ok(TldIndex::embedded()),
    }
}

/// Print how many delegated ccTLDs have RDAP routing, and which still don't.
fn report_coverage(
    iana: &ServerDirectory,
    overlays: &[Overlay],
    index: &TldIndex,
    as_json: bool,
) -> Result<()> {
    let overlay_tlds: HashSet<String> = overlays
        .iter()
        .flat_map(|overlay| overlay.servers.iter())
        .map(|entry| {
            entry
                .tld
                .trim()
                .trim_start_matches('.')
                .to_ascii_lowercase()
        })
        .collect();

    let mut delegated_tlds = 0;
    let mut cctlds = Vec::new();
    for tld in index.tlds() {
        if index.is_delegated(&tld) {
            delegated_tlds += 1;
            if index.is_cctld(&tld) {
                cctlds.push(tld);
            }
        }
    }

    let (mut via_iana, mut via_overlay) = (0, 0);
    let mut missing = Vec::new();
    for tld in &cctlds {
        if iana.contains(tld) {
            via_iana += 1;
        } else if overlay_tlds.contains(tld) {
            via_overlay += 1;
        } else {
            missing.push(tld.clone());
        }
    }

    let overlay_entries: usize = overlays.iter().map(|overlay| overlay.servers.len()).sum();
    let overlay_is_clean = overlays.iter().all(|overlay| {
        overlay
            .servers
            .iter()
            .all(|entry| index.is_cctld(&entry.tld) && index.is_delegated(&entry.tld))
    });

    if as_json {
        let document = serde_json::json!({
            "publication": index.publication(),
            "delegated_tlds": delegated_tlds,
            "delegated_cctlds": cctlds.len(),
            "cctld_with_rdap": via_iana + via_overlay,
            "via_iana": via_iana,
            "via_overlay": via_overlay,
            "without_rdap": missing.len(),
            "missing": missing,
            "overlay_entries": overlay_entries,
            "overlay_all_delegated_cctld": overlay_is_clean,
        });
        println!("{}", serde_json::to_string_pretty(&document)?);
        return Ok(());
    }

    println!(
        "RDAP coverage (index published {})",
        index.publication().unwrap_or("unknown")
    );
    println!();
    println!("delegated TLDs:   {delegated_tlds}");
    println!("delegated ccTLDs: {}", cctlds.len());
    println!("  via IANA:       {via_iana}");
    println!("  via overlay:    {via_overlay}");
    println!("  no RDAP:        {}", missing.len());
    println!();
    println!(
        "overlay: {overlay_entries} entries, all delegated ccTLDs: {}",
        if overlay_is_clean { "yes" } else { "NO" }
    );
    if !missing.is_empty() {
        println!();
        println!("ccTLDs without a known RDAP server ({}):", missing.len());
        println!("{}", missing.join(" "));
    }

    Ok(())
}
