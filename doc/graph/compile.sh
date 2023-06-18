#!/usr/bin/env bash
set -eux

ROOT=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )
cd "$ROOT"

for file in *.dot; do
    dot -Tsvg "$file" > "$file.svg"
done

