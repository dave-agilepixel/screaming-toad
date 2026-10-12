"""Check that the UI's CrawlConfig type and config schema match the Rust struct.

Usage: cargo run -q -p toad-cli -- config default.toadconfig
       python scripts/check_config_schema.py default.toadconfig
Exits non-zero on any drift.
"""
import collections
import json
import re
import sys
from pathlib import Path

cfg = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
ts = (Path(__file__).resolve().parent.parent / "app/src/config/schema.ts").read_text(encoding="utf-8")
problems = []


def interface(name):
    body = re.search(r"export interface " + name + r" \{(.*?)\n\}", ts, re.S).group(1)
    return set(re.findall(r"^\s+(\w+):", body, re.M))


for name, obj in [("CrawlConfig", cfg), ("Preferences", cfg["preferences"]), ("ContentArea", cfg["content_area"])]:
    ts_keys, rust_keys = interface(name), set(obj)
    if rust_keys - ts_keys:
        problems.append(f"{name}: fields missing from the TypeScript type: {sorted(rust_keys - ts_keys)}")
    if ts_keys - rust_keys:
        problems.append(f"{name}: TypeScript fields not in Rust: {sorted(ts_keys - rust_keys)}")

keys = re.findall(r'key: "([\w.]+)"', ts) + re.findall(r'(?:store|crawl): "(\w+)"', ts)
dupes = [k for k, n in collections.Counter(keys).items() if n > 1]
if dupes:
    problems.append(f"schema lists keys more than once: {dupes}")


def exists(path):
    o = cfg
    for part in path.split("."):
        if not isinstance(o, dict) or part not in o:
            return False
        o = o[part]
    return True


missing = [k for k in keys if not exists(k)]
if missing:
    problems.append(f"schema keys that are not config fields: {missing}")
# `mode` is set by the Mode menu; custom rules have their own editors.
unexposed = sorted(set(cfg) - {k.split(".")[0] for k in keys} - {"mode", "custom_search", "custom_extraction"})
if unexposed:
    problems.append(f"config fields with no control in the dialog: {unexposed}")

print("\n".join(problems) or "Config schema matches the Rust CrawlConfig.")
sys.exit(1 if problems else 0)
