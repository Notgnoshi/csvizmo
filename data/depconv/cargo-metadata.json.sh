#!/bin/sh
# Pruned cargo metadata for the csvizmo workspace.
# Packages trimmed to name + version + target kinds.
# Local paths replaced with ~/src/csvizmo.
cd "$(git rev-parse --show-toplevel)" || exit 1
cargo metadata --format-version=1 | jq '{
  packages: [.packages[] | {name, version, targets: [.targets[] | {name, kind}]}],
  resolve,
  version
}' | sed "s|$HOME|~|g"
