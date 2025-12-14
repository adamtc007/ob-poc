#!/usr/bin/env python3
"""
Fetch Allianz Global Investors fund lists for multiple regions.

This scrapes the fund list pages, extracts the `data-context-url` used by the
JS app (JSON list of funds/share classes), and saves the raw JSON locally.

Regions covered:
- Luxembourg (LU)
- Germany (DE)
- United Kingdom (GB)
- Ireland (IE)
- Switzerland (CH)

Outputs are written to `data/external/allianzgi/<region>_funds.json`.
"""

import re
import sys
from pathlib import Path
from typing import Dict
from urllib.parse import urljoin

import requests

PAGES: Dict[str, str] = {
    "LU": "https://regulatory.allianzgi.com/en-gb/facilities-services/luxemburg-en/funds/mutual-funds",
    "DE": "https://regulatory.allianzgi.com/de-de/b2c/deutschland-de/funds/mutual-funds",
    "GB": "https://regulatory.allianzgi.com/en-gb/b2c/united-kingdom-en/funds/mutual-funds",
    "IE": "https://regulatory.allianzgi.com/en-ie/b2c/ireland-en/funds/mutual-funds",
    "CH": "https://regulatory.allianzgi.com/de-ch/b2c/schweiz-de/funds/mutual-funds",
}

# Example: data-context-url="/en-GB/api/funddata/funds/489745d2-b1d2-44ca-8b02-02b524bdb084/ac028442-55dc-4ee1-9daf-a3109f021182"
CONTEXT_PATTERN = re.compile(
    r'data-context-url="(?P<path>[^"]*/api/funddata/funds/[^"]+)"', re.IGNORECASE
)
OUT_DIR = Path("data/external/allianzgi")


def fetch_html(url: str) -> str:
    resp = requests.get(url, timeout=30)
    resp.raise_for_status()
    return resp.text


def extract_context_url(html: str, base_url: str) -> str:
    match = CONTEXT_PATTERN.search(html)
    if not match:
        raise RuntimeError("No fund data context URL found in page")
    path = match.group("path")
    return urljoin(base_url, path)


def fetch_and_save(region: str, page_url: str) -> Path:
    html = fetch_html(page_url)
    context_url = extract_context_url(html, page_url)

    resp = requests.get(context_url, timeout=60)
    resp.raise_for_status()

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    out_path = OUT_DIR / f"{region.lower()}_funds.json"
    out_path.write_bytes(resp.content)
    return out_path


def main(regions=None) -> int:
    regions = regions or list(PAGES.keys())
    for region in regions:
        page_url = PAGES.get(region.upper())
        if not page_url:
            print(f"[warn] Unknown region: {region}", file=sys.stderr)
            continue
        try:
            out = fetch_and_save(region.upper(), page_url)
            print(f"[ok] {region.upper()}: saved {out}")
        except Exception as exc:
            print(f"[err] {region.upper()}: {exc}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
