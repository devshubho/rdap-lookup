#!/usr/bin/env python3
"""Vendor the IANA sources this project builds from into ``data/official/``.

Re-fetches each upstream file, stores the bytes verbatim (the Root Zone
Database is gzipped), and writes a ``manifest.json`` recording the source URL,
content sha256 and byte count for each. The committed snapshot makes the build
offline and reproducible and turns upstream changes into a reviewable diff.

Run this to refresh; then regenerate with ``scripts/gen-tlds.py`` and
``rdap-lookup --emit-bootstrap``.
"""

from __future__ import annotations

import argparse
import datetime as dt
import gzip
import hashlib
import json
import os
import urllib.request

# local filename -> (source URL, storage)
SOURCES = {
    "iana-rdap-bootstrap.json": ("https://data.iana.org/rdap/dns.json", "plain"),
    "iana-root-zone-tlds.txt": (
        "https://data.iana.org/TLD/tlds-alpha-by-domain.txt",
        "plain",
    ),
    "iana-root-zone-db.html": ("https://www.iana.org/domains/root/db", "gzip"),
}


def fetch(url: str) -> bytes:
    request = urllib.request.Request(
        url, headers={"User-Agent": "rdap-lookup-fetch-official"}
    )
    with urllib.request.urlopen(request, timeout=120) as response:
        return response.read()


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--dir", default="data/official", help="output directory")
    parser.add_argument("--fetched-at", help="override the manifest timestamp")
    args = parser.parse_args()

    os.makedirs(args.dir, exist_ok=True)
    fetched_at = args.fetched_at or dt.datetime.now(dt.UTC).strftime("%Y-%m-%dT%H:%M:%SZ")

    files = []
    for name, (url, storage) in SOURCES.items():
        data = fetch(url)
        if storage == "gzip":
            path = os.path.join(args.dir, name + ".gz")
            with open(path, "wb") as handle:
                handle.write(gzip.compress(data, mtime=0))
        else:
            path = os.path.join(args.dir, name)
            with open(path, "wb") as handle:
                handle.write(data)
        files.append(
            {
                "path": os.path.relpath(path, args.dir).replace(os.sep, "/"),
                "url": url,
                "sha256": hashlib.sha256(data).hexdigest(),
                "bytes": len(data),
            }
        )
        print(f"fetched {url} -> {path} ({len(data)} bytes)")

    manifest = {"fetched_at": fetched_at, "sources": files}
    manifest_path = os.path.join(args.dir, "manifest.json")
    with open(manifest_path, "w", encoding="utf-8", newline="\n") as handle:
        json.dump(manifest, handle, indent=2)
        handle.write("\n")
    print(f"wrote {manifest_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
