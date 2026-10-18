# Contributing

Thanks for helping keep RDAP coverage complete.

## Adding or fixing a ccTLD RDAP server

The most useful contribution is a correction to `data/rdap-cctld-servers.json`.

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

## Development

```sh
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
```

`data/tlds.json` is generated from IANA; regenerate it with
`python scripts/gen-tlds.py`.

Commit messages follow [Conventional Commits](https://www.conventionalcommits.org/).
