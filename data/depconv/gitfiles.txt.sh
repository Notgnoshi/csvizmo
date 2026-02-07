#!/bin/sh
cd "$(git rev-parse --show-toplevel)" || exit 1
git ls-files crates/
