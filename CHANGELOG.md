# Changelog

All notable changes to this project are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Initial library: IANA bootstrap parsing, supplemental ccTLD overlay merge,
  IDN-to-A-label resolution, and an RDAP domain client.
- `rdap-lookup` CLI for resolving a domain to its RDAP server and querying it.
- On-disk `ETag` / `Last-Modified` bootstrap cache.
- `--emit-bootstrap <PATH>` and `ServerDirectory::to_bootstrap_json` to export
  the merged directory as a single RFC 7484 bootstrap file.
- Generated `data/generated/tlds.json` classification index with `TldIndex`
  (`kind` / `is_cctld` / `is_delegated`), derived from IANA's Root DB.
- `rdap-lookup --coverage` (and `--json`) reporting delegated-ccTLD RDAP
  coverage and the remaining gaps.
- `is_idn` and `to_unicode` helpers for IDN A-label/U-label handling.
- `scripts/gen-tlds.py` generates `data/generated/tlds.json` straight from IANA's Root
  Zone Database and root zone TLD list, replacing the previous third-party
  source. Overlay entries now carry their own `evidence`; the 19 ccTLD servers
  were located and probed independently.

### Security

- Upgraded `time` to 0.3.47 to fix a stack-exhaustion advisory; this raises the
  minimum supported Rust version to 1.88.
