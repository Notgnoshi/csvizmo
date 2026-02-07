#!/bin/sh
cd "$(git rev-parse --show-toplevel)" || exit 1
# Default cargo tree output for csvizmo-depgraph crate, with the path to the csvizmo project removed
cargo tree -p csvizmo-depgraph | sed "s|$HOME/.*/csvizmo/|csvizmo/|g"
