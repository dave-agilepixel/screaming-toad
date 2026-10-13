"""Generate crates/toad-core/data/schemaorg.json from the schema.org vocabulary.

Usage:
  curl -sSL -o vocab.jsonld https://schema.org/version/latest/schemaorg-current-https.jsonld
  python scripts/gen_schemaorg.py vocab.jsonld

Output: {"version": ..., "types": {Type: [superclasses]}, "properties": {prop: [domain types]}}.
Used by Structured Data > Schema.org Validation. Schema.org is CC BY-SA 3.0.
"""
import json
import sys
from pathlib import Path


def ids(v):
    if v is None:
        return []
    items = v if isinstance(v, list) else [v]
    return [i["@id"] if isinstance(i, dict) else i for i in items]


def short(i):
    return i.split(":", 1)[1] if i.startswith("schema:") else None


src = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
types, props = {}, {}
for node in src["@graph"]:
    t = node.get("@type")
    t = t if isinstance(t, list) else [t]
    name = short(node.get("@id", ""))
    if not name:
        continue
    if "rdfs:Class" in t:
        types[name] = sorted(s for s in (short(x) for x in ids(node.get("rdfs:subClassOf"))) if s)
    elif "rdf:Property" in t:
        props[name] = sorted(s for s in (short(x) for x in ids(node.get("schema:domainIncludes"))) if s)

out = {"source": "https://schema.org/version/latest/schemaorg-current-https.jsonld", "types": dict(sorted(types.items())), "properties": dict(sorted(props.items()))}
dest = Path(__file__).resolve().parent.parent / "crates/toad-core/data/schemaorg.json"
dest.write_text(json.dumps(out, separators=(",", ":")), encoding="utf-8")
print(f"{len(types)} types, {len(props)} properties -> {dest} ({dest.stat().st_size // 1024} KB)")
