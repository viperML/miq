#!/usr/bin/env bash
set -eux

VENDOR="$( cd "$(dirname "$BASH_SOURCE")"; pwd )"
cd "$VENDOR"

if [[ ! -f "$VENDOR/bash" ]]; then
    nix build "$VENDOR"#pkgsStatic.bash^out -o result-bash
    ln -vs result-bash/bin/bash bash
fi

if [[ ! -f "$VENDOR/busybox" ]]; then
    nix build "$VENDOR"#pkgsStatic.busybox^out -o result-busybox
    ln -vs result-busybox/bin/busybox busybox
fi
