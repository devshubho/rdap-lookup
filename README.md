# RDAP-Server-Lookup

Curated RDAP server directory — IANA's bootstrap plus hand-maintained ccTLD servers — served from one stable URL, with a Rust client for domain lookups.

RDAP domain lookups that actually cover ccTLDs: one source, no gaps.

## Why

IANA publishes canonical RDAP routing in its [RFC 7484 bootstrap file](https://data.iana.org/rdap/dns.json), but it lists only some ccTLDs — each ccTLD is self-governed, so most are absent. This project adds a small, hand-maintained overlay for those gaps and exposes both through a single interface.

The overlay is a **delta, not a copy**: IANA always wins, and an overlay entry is used only when IANA does not already cover the TLD. When IANA adds a ccTLD itself, the overlay entry silently defers — no stale overrides.

## Use it

```sh
# Resolve the server and fetch the RDAP record
rdap-lookup example.com

# Resolve the server only
rdap-lookup --server-only nic.ch

# Show delegated-ccTLD RDAP coverage and the remaining gaps
rdap-lookup --coverage
rdap-lookup --coverage --json   # machine-readable
```

As a library:

```rust
use rdap_lookup::{Cache, IANA_BOOTSTRAP_URL, Overlay, RdapClient, ServerDirectory};

# async fn run() -> anyhow::Result<()> {
let cache = Cache::new(std::env::temp_dir().join("rdap-lookup"));
let iana = cache.get_or_fetch(IANA_BOOTSTRAP_URL).await?;

let mut directory = ServerDirectory::from_iana(&iana)?;
let overlay: Overlay = serde_json::from_str(include_str!("data/rdap-servers.json"))?;
directory.merge_overlay(&overlay);

let tld = rdap_lookup::resolve_tld("пример.рф")?;    // "xn--p1ai"
let domain = rdap_lookup::to_ascii("пример.рф")?;    // "xn--e1afmkfd.xn--p1ai"
let server = directory.servers_for(&tld).and_then(|s| s.first()).unwrap();

let record = RdapClient::new().lookup(server, &domain).await?;
# let _ = record;
# Ok(())
# }
```

## Publish one stable URL

Merge IANA's bootstrap with the overlay into a single RFC 7484 file and serve it
from anywhere:

```sh
# Write the merged directory to a file (`-` prints to stdout)
rdap-lookup --emit-bootstrap dns.json
```

The merged directory is published at
<https://data.rdapserver.org/dns.json>, so an RDAP client only needs one URL.
The raw overlay is published alongside it at
<https://data.rdapserver.org/rdap-servers.json>. TLDs sharing a server are
grouped, and IANA entries always win.

## ccTLD RDAP servers

ICANN governs the gTLDs and requires each to run an RDAP server. The roughly 250
ccTLDs each govern themselves, so every one decides on its own whether to run
RDAP, WHOIS, or neither. IANA's bootstrap lists only some of them — the rest are
the gaps this project fills.

Each overlay entry records where its URL came from (`source`) and when it was
last confirmed (`date_updated`): usually an official page, sometimes a
best-effort guess. Because ccTLD URLs move, a
[scheduled check](.github/workflows/monitor-cctld-rdap.yaml) probes the servers
we list so stale entries surface early.

### Coverage

`rdap-lookup --coverage` joins a generated classification index
(`data/tlds.json`, derived from IANA's Root DB and the root-zone TLD list) with
the bootstrap and overlay. It reports how many delegated ccTLDs have RDAP
routing — via IANA or via our overlay — and lists the ones that still don't,
which is the work queue for new overlay entries. `--tlds <PATH>` overrides the
embedded index.

## The data

| File | Purpose |
| --- | --- |
| `data/rdap-servers.json` | Supplemental ccTLD RDAP servers IANA omits. |

The file carries a top-level `publication` timestamp (ISO 8601 UTC, e.g. `2026-10-18T07:24:09Z`), mirroring IANA's bootstrap. Each server entry carries `tld`, `rdap_server`, `backend_operator`, `date_updated`, and `source` (evidence). See [CONTRIBUTING.md](CONTRIBUTING.md) to add one.

Resolution rules:

- TLDs are matched by their ASCII **A-label** form; IDNs are punycoded before lookup.
- Precedence: **IANA bootstrap → supplemental overlay**.
- When no server is known, callers fall back to WHOIS (see `whois_server` in a full `tlds.json`).

## Development

```sh
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
```

## License

MIT for the software (see [LICENSE](LICENSE)). Supplemental data is contributed
under CC0-1.0. IANA and iana-data attribution is in [NOTICE](NOTICE). IANA does
not endorse this project.
