# Security Policy

## Reporting a vulnerability

Please report suspected vulnerabilities privately via GitHub's
[Security Advisories](https://github.com/devshubho/rdap-server/security/advisories/new)
rather than a public issue. We aim to acknowledge reports within a few days.

## Scope

This tool fetches a JSON bootstrap file over HTTPS and issues RDAP queries to
third-party servers. Note that RDAP responses are untrusted input and are
returned to callers as JSON; validate before acting on them.

The supplemental overlays are data, not code, but a malicious entry could point
a TLD at an attacker-controlled server. Changes to
`data/manual/cctld-rdap-servers.json` and `data/manual/punycode-rdap-servers.json`
are reviewed and must cite a source.
