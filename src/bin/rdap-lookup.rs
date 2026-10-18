//! `rdap-lookup` CLI: resolve a domain to its RDAP server and (optionally)
//! fetch its RDAP record.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::Parser;
use rdap_lookup::{
    Cache, IANA_BOOTSTRAP_URL, Overlay, RdapClient, ServerDirectory, resolve_tld, to_ascii,
};
use tracing_subscriber::EnvFilter;

/// Resolve a domain to its RDAP server and query it.
#[derive(Debug, Parser)]
#[command(name = "rdap-lookup", version, about, long_about = None)]
struct Cli {
    /// Domain to look up, e.g. `example.com` or `nic.ch`.
    domain: Option<String>,

    /// Supplemental ccTLD overlay JSON.
    #[arg(long, default_value = "data/rdap-servers.json")]
    overlay: PathBuf,

    /// Directory for the cached IANA bootstrap.
    #[arg(long, env = "RDAP_LOOKUP_CACHE")]
    cache_dir: Option<PathBuf>,

    /// IANA bootstrap URL (override for self-hosting or testing).
    #[arg(long, default_value_t = String::from(IANA_BOOTSTRAP_URL))]
    bootstrap_url: String,

    /// Print the RDAP server URL without querying it.
    #[arg(long)]
    server_only: bool,

    /// Merge IANA + overlay and write a single RFC 7484 bootstrap JSON to
    /// PATH (use `-` for stdout) instead of looking up a domain.
    #[arg(long, value_name = "PATH")]
    emit_bootstrap: Option<PathBuf>,
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
    let iana = cache
        .get_or_fetch(&cli.bootstrap_url)
        .await
        .context("fetching IANA RDAP bootstrap")?;
    let mut directory = ServerDirectory::from_iana(&iana).context("parsing IANA RDAP bootstrap")?;

    let mut overlay_publication: Option<String> = None;
    if cli.overlay.exists() {
        let raw = std::fs::read_to_string(&cli.overlay)
            .with_context(|| format!("reading overlay {}", cli.overlay.display()))?;
        let overlay: Overlay = serde_json::from_str(&raw)
            .with_context(|| format!("parsing overlay {}", cli.overlay.display()))?;
        overlay_publication = overlay.publication().map(str::to_owned);
        let added = directory.merge_overlay(&overlay);
        match overlay.publication() {
            Some(publication) => {
                tracing::info!("overlay published {publication}; added {added} server(s)")
            }
            None => tracing::info!("overlay added {added} supplemental server(s)"),
        }
    } else {
        tracing::warn!(
            "overlay {} not found; using IANA bootstrap only",
            cli.overlay.display()
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
