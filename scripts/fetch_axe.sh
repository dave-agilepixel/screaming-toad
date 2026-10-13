#!/usr/bin/env sh
# Refresh the bundled axe-core (MPL-2.0) used by Spider > Extraction >
# Accessibility. Usage: scripts/fetch_axe.sh [version]
set -e
v="${1:-4.10.3}"
tmp="$(mktemp -d)"
(cd "$tmp" && npm pack "axe-core@$v" --silent >/dev/null && tar -xzf "axe-core-$v.tgz")
cp "$tmp/package/axe.min.js" "$(dirname "$0")/../crates/toad-core/data/axe.min.js"
rm -rf "$tmp"
echo "axe-core $v -> crates/toad-core/data/axe.min.js"
