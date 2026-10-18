# Data

Three tiers with a one-way flow: `official` + `manual` → `generated` →
published (`site/`).

```
official/     verbatim upstream snapshots (IANA). Never hand-edited.
manual/       our hand-authored inputs. Edited in pull requests.
generated/    machine output. Never hand-edited.
```

## `official/`

Fetched byte-for-byte from IANA and committed, so the build is offline and
reproducible. Refresh with `python scripts/fetch-official.py`, which also
rewrites `manifest.json` (source URL, sha256, fetched-at).

| File | Source |
| --- | --- |
| `iana-rdap-bootstrap.json` | https://data.iana.org/rdap/dns.json |
| `iana-root-zone-tlds.txt` | https://data.iana.org/TLD/tlds-alpha-by-domain.txt |
| `iana-root-zone-db.html.gz` | https://www.iana.org/domains/root/db |

## `manual/`

| File | Purpose |
| --- | --- |
| `cctld-rdap-servers.json` | The hand-maintained ccTLD overlay (a delta; IANA always wins). |
| `punycode-rdap-servers.json` | The same overlay for IDN ccTLDs, keyed by A-label (`xn--`). |

Every `*.json` in this directory is merged (in filename order), so a new file
here is picked up with no code change.

See [../CONTRIBUTING.md](../CONTRIBUTING.md).

## `generated/`

| File | Built by | From |
| --- | --- | --- |
| `tlds.json` | `python scripts/gen-tlds.py` | `official/` |
| `dns.json` | `rdap-lookup --iana data/official/iana-rdap-bootstrap.json --emit-bootstrap` | `official/` + `manual/` |

Never edit these by hand — regenerate them. The publication timestamp is taken
from the snapshot manifest (or the overlay), so regenerating from the same
inputs is byte-for-byte reproducible.

## Publishing

`site/dns.json`, `site/rdap-cctld-servers.json`, and `site/punycode-rdap-servers.json`
are verbatim copies of `generated/dns.json`, `manual/cctld-rdap-servers.json`, and
`manual/punycode-rdap-servers.json`. Tests fail when they drift, so always copy
after regenerating.
