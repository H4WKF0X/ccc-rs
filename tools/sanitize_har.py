#!/usr/bin/env python3
"""Strip secrets from a browser HAR so the API shape can be shared (e.g. with Claude).

    python3 tools/sanitize_har.py [docs/whatever.har] [-o docs/har-summary.json]

Keeps, for each codingcontest.org request (static assets skipped): method, URL with signed-URL
parameters removed, status, request header *names*, the auth scheme word (e.g. "Bearer"),
content type, request body and JSON response body, with JWTs and "token" fields replaced.
Drops all header values, cookies and non-JSON response bodies. Review the output before sharing.
"""

import argparse
import glob
import json
import re

STATIC = re.compile(r"\.(js|css|woff2?|ttf|png|svg|ico|jpe?g|webp)(\?|$)")
SIGNED_PARAMS = re.compile(r"\b(sig|skoid|sktid|skt|ske|sks|skv|st|se|token)=[^&\"\s]+")
JWT = re.compile(r"eyJ[\w-]+\.[\w-]+\.[\w-]+")
TOKEN_FIELD = re.compile(r"\"(token|accessToken|refreshToken|sessionId)\"\s*:\s*\"[^\"]+\"")
LIMIT = 3000


def scrub(text):
    text = SIGNED_PARAMS.sub(r"\1=<X>", text or "")
    text = JWT.sub("<JWT>", text)
    return TOKEN_FIELD.sub(r'"\1":"<X>"', text)


AUTH_SCHEMES = {"bearer", "basic", "token", "digest"}
# Header values that identify the request shape and are not secrets.
SAFE_HEADER_VALUES = {"x-ccc-slug", "origin", "referer", "accept", "content-type", "sec-fetch-site"}


def auth_scheme(value):
    """Only ever a known scheme word; a raw token (no scheme) must not leak."""
    if not value:
        return None
    word = value.split(" ")[0]
    return word if word.lower() in AUTH_SCHEMES else "<raw token, no scheme word>"


def summarize(entry):
    req, resp = entry["request"], entry["response"]
    headers = {h["name"].lower(): h["value"] for h in req["headers"]}
    mime = resp["content"].get("mimeType") or ""
    return {
        "method": req["method"],
        "url": scrub(req["url"]),
        "status": resp["status"],
        "req_headers": sorted(h for h in headers if not h.startswith(":")),
        "safe_header_values": {h: scrub(v) for h, v in headers.items() if h in SAFE_HEADER_VALUES},
        "auth_scheme": auth_scheme(headers.get("authorization")),
        "req_type": headers.get("content-type"),
        "req_body": scrub((req.get("postData") or {}).get("text", ""))[:LIMIT],
        "resp_type": mime,
        "resp": scrub(resp["content"].get("text", ""))[:LIMIT] if "json" in mime else None,
    }


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("har", nargs="?", help="default: the first docs/*.har")
    ap.add_argument("-o", "--out", default="docs/har-summary.json")
    args = ap.parse_args()
    path = args.har or sorted(glob.glob("docs/*.har"))[0]
    with open(path, encoding="utf-8") as f:
        entries = json.load(f)["log"]["entries"]
    out = [
        summarize(e)
        for e in entries
        if "codingcontest.org" in e["request"]["url"] and not STATIC.search(e["request"]["url"])
    ]
    with open(args.out, "w", encoding="utf-8") as f:
        json.dump(out, f, indent=1)
    print(f"{len(out)} requests -> {args.out}  (review it before sharing)")


if __name__ == "__main__":
    main()
