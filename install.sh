#!/usr/bin/env bash
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PARENT_DIR="$(dirname "$SCRIPT_DIR")"
CORE_DIR="$PARENT_DIR/synapt-core"

if [ -d "$CORE_DIR" ]; then
    echo "synapt-core already present at $CORE_DIR"
else
    echo "Cloning synapt-core..."
    git clone https://github.com/aatishbagal/synapt-core.git "$CORE_DIR"
    echo "synapt-core cloned."
fi

echo "Install complete. Run: cargo tauri dev"
