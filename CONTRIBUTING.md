# Contributing

Thanks for helping keep RDAP coverage complete.

## Adding or fixing a ccTLD RDAP server

The most useful contribution is a correction to `data/rdap-servers.json`.

1. Add or edit an entry:
   ```json
   {
     "tld": "example",
     "rdap_server": "https://rdap.example/",
     "backend_operator": "Example Registry",
     "date_updated": "YYYY-MM-DD",
     "source": "https://link/to/evidence"
   }
   ```
2. The server must answer `GET {rdap_server}domain/nic.{tld}` with HTTP 200,
   `content-type: application/rdap+json`, and `$.objectClassName == "domain"`.
3. IANA always wins. Overlay entries are used only for TLDs IANA's bootstrap
   does not cover, so a duplicate is harmless but will be ignored.

## Development

```sh
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
```

Commit messages follow [Conventional Commits](https://www.conventionalcommits.org/).
