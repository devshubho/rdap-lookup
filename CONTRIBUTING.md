# Contributing

Thanks for helping keep RDAP coverage complete.

## Adding or fixing a ccTLD RDAP server

The most useful contribution is a correction to the overlay. Put ASCII ccTLDs in
`data/manual/cctld-rdap-servers.json` and IDN ccTLDs (A-labels, `xn--…`) in
`data/manual/punycode-rdap-servers.json`. Every `*.json` in `data/manual/` is
merged, so the split is organisational only.

1. Add or edit an entry:
   ```json
   {
     "tld": "example",
     "rdap_server": "https://rdap.example/",
     "backend_operator": "Example Registry",
     "date_updated": "YYYY-MM-DD",
     "source": "https://official/reference",
     "evidence": "GET https://rdap.example/domain/nic.example -> 200 application/rdap+json"
   }
   ```
   `source` is an optional official reference; `evidence` records the
   observation that confirmed the server.
2. The server must answer `GET {rdap_server}domain/nic.{tld}` with HTTP 200,
   `content-type: application/rdap+json`, and `$.objectClassName == "domain"`.
   Paste that result into `evidence` and set `date_updated` to the day you
   checked it.
3. Bump the top-level `publication` timestamp to the current UTC time
   (`YYYY-MM-DDTHH:MM:SSZ`).
4. IANA always wins. Overlay entries are used only for TLDs IANA's bootstrap
   does not cover, so a duplicate is harmless but will be ignored.
5. Sync the published copies and the merged product:

   ```sh
   cp data/manual/cctld-rdap-servers.json site/rdap-cctld-servers.json
   cp data/manual/punycode-rdap-servers.json site/punycode-rdap-servers.json
   rdap-lookup --iana data/official/iana-rdap-bootstrap.json \
       --emit-bootstrap data/generated/dns.json
   cp data/generated/dns.json site/dns.json
   ```

   Tests assert `site/` is byte-identical to the sources, so a missed copy
   fails CI.

## Development

```sh
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
```

The `data/` tiers are `official/` (verbatim IANA snapshots, refresh with
`python scripts/fetch-official.py`), `manual/` (hand-edited), and `generated/`
(machine output, e.g. `python scripts/gen-tlds.py`). Never hand-edit `official/`
or `generated/`; see [data/README.md](data/README.md).

Commit messages follow [Conventional Commits](https://www.conventionalcommits.org/).
