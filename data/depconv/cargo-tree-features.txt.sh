#!/bin/sh
# Feature-annotated cargo tree, depth-limited to 2. Path to the csvizmo project removed
cd "$(git rev-parse --show-toplevel)" || exit 1
cargo tree -p csvizmo-depgraph -e features --depth 2 | sed "s|$HOME/.*/csvizmo/|csvizmo/|g"
