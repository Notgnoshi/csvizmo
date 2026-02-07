#!/bin/sh
cd "$(git rev-parse --show-toplevel)" || exit 1
find ./crates -name '*.rs' | sort
