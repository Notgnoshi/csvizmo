#!/bin/sh
# Full cargo metadata for the csvizmo workspace with local paths replaced.
cd "$(git rev-parse --show-toplevel)" || exit 1
cargo metadata --format-version=1 | sed "s|$HOME|~|g" | jq --indent 4
