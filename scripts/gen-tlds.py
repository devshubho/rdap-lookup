#!/usr/bin/env python3
"""Regenerate ``data/tlds.json`` from IANA primary sources.

The index answers two questions the RDAP bootstrap cannot: which TLDs are
ccTLDs, and which are actually delegated. Both facts come straight from IANA,
so no third-party dataset is copied:

* type per TLD    -> IANA Root Zone Database   https://www.iana.org/domains/root/db
* delegation      -> root zone TLD list        https://data.iana.org/TLD/tlds-alpha-by-domain.txt

Only the standard library is used, so it runs anywhere without installing
anything. Pass local paths to ``--root-db`` / ``--zone`` to work offline.

Usage:
    python scripts/gen-tlds.py                       # fetch from IANA
    python scripts/gen-tlds.py --root-db page.html --zone tlds.txt -o data/tlds.json
"""

from __future__ import annotations

import argparse
import datetime as dt
import re
import sys
import urllib.request

ROOT_DB_URL = "https://www.iana.org/domains/root/db"
ZONE_URL = "https://data.iana.org/TLD/tlds-alpha-by-domain.txt"
OUTPUT = "data/tlds.json"

# IANA's "Type" column -> our Kind (see src/classify.rs).
TYPE_TO_KIND = {
    "country code": "cctld",
    "generic": "gtld",
    "sponsored": "gtld",
    "generic-restricted": "gtld",
    "special use": "infrastructure",
    "infrastructure": "infrastructure",
}

ROW_RE = re.compile(r"<tr>(.*?)</tr>", re.S)
TLD_RE = re.compile(r'domains/root/db/([a-z0-9-]+)"')
CELL_RE = re.compile(r"<td[^>]*>(.*?)</td>", re.S)
TAG_RE = re.compile(r"<[^>]+>")
WS_RE = re.compile(r"\s+")


def read(source: str) -> str:
    """Read a URL or a local path into text."""
    if source.startswith(("http://", "https://")):
        with urllib.request.urlopen(source, timeout=60) as response:
            return response.read().decode("utf-8")
    with open(source, encoding="utf-8") as handle:
        return handle.read()


def parse_types(html: str) -> dict[str, str]:
    """Map each TLD in the Root Zone Database to our Kind."""
    kinds: dict[str, str] = {}
    for row in ROW_RE.findall(html):
        match = TLD_RE.search(row)
        if not match:
            continue
        cells = [
            WS_RE.sub(" ", TAG_RE.sub(" ", cell)).strip()
            for cell in CELL_RE.findall(row)
        ]
        if len(cells) < 3:
            continue
        kind = TYPE_TO_KIND.get(cells[2].lower())
        if kind is None:
            print(f"warning: unknown IANA type {cells[2]!r}", file=sys.stderr)
            continue
        kinds[match.group(1).lower()] = kind
    return kinds


def parse_zone(text: str) -> set[str]:
    """The set of currently delegated TLDs (comments start with ``#``)."""
    return {
        line.strip().lower()
        for line in text.splitlines()
        if line.strip() and not line.startswith("#")
    }


def build(kinds: dict[str, str], zone: set[str], publication: str) -> str:
    entries = [
        {"tld": tld, "type": kinds[tld], "delegated": tld in zone}
        for tld in sorted(kinds)
    ]
    lines = [
        "{",
        '  "version": "1",',
        f'  "publication": "{publication}",',
        f'  "source": "IANA Root Zone Database + root zone TLD list ({ROOT_DB_URL}, {ZONE_URL})",',
        '  "tlds": [',
    ]
    for index, entry in enumerate(entries):
        comma = "," if index < len(entries) - 1 else ""
        delegated = "true" if entry["delegated"] else "false"
        lines.append(
            f'    {{"tld":"{entry["tld"]}","type":"{entry["type"]}",'
            f'"delegated":{delegated}}}{comma}'
        )
    lines += ["  ]", "}"]
    return "\n".join(lines) + "\n"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root-db", default=ROOT_DB_URL, help="Root Zone Database URL or path")
    parser.add_argument("--zone", default=ZONE_URL, help="root zone TLD list URL or path")
    parser.add_argument("-o", "--output", default=OUTPUT, help="output path")
    parser.add_argument("--publication", help="override the publication timestamp")
    args = parser.parse_args()

    kinds = parse_types(read(args.root_db))
    zone = parse_zone(read(args.zone))
    publication = args.publication or dt.datetime.now(dt.UTC).strftime("%Y-%m-%dT%H:%M:%SZ")

    with open(args.output, "w", encoding="utf-8", newline="\n") as handle:
        handle.write(build(kinds, zone, publication))

    delegated = sum(1 for kind in kinds if kind in zone)
    cctlds = sum(1 for tld, kind in kinds.items() if kind == "cctld")
    print(
        f"wrote {args.output}: {len(kinds)} TLDs "
        f"({cctlds} ccTLDs, {delegated} delegated, {len(kinds) - delegated} undelegated)"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
