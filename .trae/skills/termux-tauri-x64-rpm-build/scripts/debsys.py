#!/usr/bin/env python3
"""Build a Debian amd64 sysroot for cross-linking a Tauri app on Termux/Android.

Resolves the runtime+dev dependency closure of the configured seed packages
from the Debian package index, downloads the .deb files and extracts them into
a sysroot directory.

Usage:
  ./debsys.py --list                      # resolve closure only
  ./debsys.py                             # download + extract
  ./debsys.py --mirror https://mirrors.ustc.edu.cn/debian --sysroot ~/xsys
"""
import argparse
import gzip
import io
import lzma
import os
import re
import sys
import tarfile
import time
import urllib.request

DEFAULT_SEEDS = [
    "libc6-dev",
    "libgcc-s1",
    "libgcc-14-dev",
    "libgtk-3-dev",
    "libwebkit2gtk-4.1-dev",
    "libsoup-3.0-dev",
    "librsvg2-dev",
    "libssl-dev",  # reqwest default-tls pulls openssl-sys
]

DEP_RE = re.compile(r"^([A-Za-z0-9][A-Za-z0-9._+\-]*)(?::[A-Za-z0-9_\-]+)?(?:\s*\([^)]*\))?$")


def log(*a):
    print(*a, flush=True)


def fetch(url, retries=4):
    last = None
    for i in range(retries):
        try:
            req = urllib.request.Request(url, headers={"User-Agent": "curl/8"})
            with urllib.request.urlopen(req, timeout=60) as r:
                return r.read()
        except Exception as e:  # noqa: BLE001
            last = e
            time.sleep(2 + i * 3)
    raise last


def load_index(mirror, suite, cache):
    path = os.path.join(cache, "Packages.xz")
    if not os.path.exists(path):
        log("downloading package index ...")
        os.makedirs(cache, exist_ok=True)
        url = f"{mirror}/dists/{suite}/main/binary-amd64/Packages.xz"
        with open(path, "wb") as f:
            f.write(fetch(url))
    with lzma.open(path, "rt", encoding="utf-8", errors="replace") as f:
        content = f.read()

    pkgs, provides = {}, {}
    for stanza in content.split("\n\n"):
        if not stanza.strip():
            continue
        fields, cur = {}, None
        for line in stanza.splitlines():
            if line.startswith(" ") and cur:
                fields[cur] += "\n" + line.strip()
                continue
            if ":" in line:
                k, v = line.split(":", 1)
                cur = k.strip()
                fields[cur] = v.strip()
        name = fields.get("Package")
        if not name:
            continue
        entry = {
            "name": name,
            "filename": fields.get("Filename", ""),
            "depends": fields.get("Depends", ""),
            "pre": fields.get("Pre-Depends", ""),
            "provides": fields.get("Provides", ""),
            "version": fields.get("Version", ""),
        }
        pkgs.setdefault(name, entry)
        for prov in entry["provides"].split(","):
            prov = prov.strip().split("(")[0].strip()
            if prov:
                provides.setdefault(prov, []).append(name)
    return pkgs, provides


def dep_names(field):
    out = []
    for clause in field.split(","):
        for alt in clause.split("|"):
            m = DEP_RE.match(alt.strip())
            if m:
                out.append(m.group(1))
                break
    return out


def resolve(pkgs, provides, seeds):
    chosen, seen, queue = {}, set(), list(seeds)
    while queue:
        name = queue.pop(0)
        if name in seen:
            continue
        seen.add(name)
        entry = pkgs.get(name)
        if entry is None:
            providers = provides.get(name)
            if providers:
                entry = pkgs[providers[0]]
                name = entry["name"]
            if entry is None:
                log(f"  ! missing package: {name}")
                continue
        if not entry["filename"] or name in chosen:
            continue
        chosen[name] = entry
        queue.extend(dep_names(entry["depends"]))
        queue.extend(dep_names(entry["pre"]))
    return chosen


def deb_member(data):
    if not data.startswith(b"!<arch>\n"):
        raise ValueError("not an ar archive")
    pos = 8
    while pos + 60 <= len(data):
        name = data[pos : pos + 16].decode().strip()
        size = int(data[pos + 48 : pos + 58].decode().strip())
        body_start = pos + 60
        body = data[body_start : body_start + size]
        if name.startswith("data.tar"):
            return name, body
        pos = body_start + size + (size % 2)
    raise ValueError("data.tar not found")


def extract_deb(path, sysroot):
    with open(path, "rb") as f:
        data = f.read()
    name, body = deb_member(data)
    if name.endswith(".xz"):
        raw = lzma.decompress(body)
    elif name.endswith(".zst"):
        from compression import zstd  # Python >= 3.14

        raw = zstd.decompress(body)
    elif name.endswith(".gz"):
        raw = gzip.decompress(body)
    else:
        raw = body
    # Seekable stream is required: Debian payloads contain hard links.
    with tarfile.open(fileobj=io.BytesIO(raw), mode="r:") as tar:
        tar.extractall(sysroot, filter="fully_trusted")


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--mirror", default=os.environ.get("DEBIAN_MIRROR", "https://mirrors.tuna.tsinghua.edu.cn/debian"))
    ap.add_argument("--suite", default=os.environ.get("DEBIAN_SUITE", "trixie"))
    ap.add_argument("--sysroot", default=os.path.expanduser(os.environ.get("XSYS_ROOT", "~/xsys")))
    ap.add_argument("--cache", default=os.path.expanduser("~/xbuild/deb-cache"))
    ap.add_argument("--seed", action="append", help="extra seed package (repeatable)")
    ap.add_argument("--list", action="store_true", help="resolve the closure only")
    args = ap.parse_args()

    seeds = DEFAULT_SEEDS + (args.seed or [])
    pkgs, provides = load_index(args.mirror, args.suite, args.cache)
    log(f"index: {len(pkgs)} packages")
    chosen = resolve(pkgs, provides, seeds)
    log(f"closure: {len(chosen)} packages")
    if args.list:
        for entry in sorted(chosen.values(), key=lambda e: e["name"]):
            log(f"  {entry['name']} {entry['version']}")
        return 0

    os.makedirs(args.sysroot, exist_ok=True)
    total = 0
    for i, entry in enumerate(sorted(chosen.values(), key=lambda e: e["name"]), 1):
        path = os.path.join(args.cache, os.path.basename(entry["filename"]))
        if not os.path.exists(path):
            log(f"[{i}/{len(chosen)}] {entry['name']} {entry['version']}")
            with open(path, "wb") as f:
                f.write(fetch(f"{args.mirror}/{entry['filename']}"))
        total += os.path.getsize(path)
        extract_deb(path, args.sysroot)
    log(f"done: {len(chosen)} debs, {total / 1048576:.0f} MiB cached, sysroot={args.sysroot}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
