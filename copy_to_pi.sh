#!/usr/bin/env bash
set -euo pipefail

SRC_DIR="/home/outofrange/Projects/PlainSight/"
DEST_HOST="god@cube"
DEST_DIR="Projects/PlainSight"

# Transfer (excluding unwanted files)
rsync -avz --progress \
  --exclude 'target/' \
  --exclude 'copy_to_pi.sh' \
  --exclude 'Cargo.lock' \
  --exclude '.git' \
  "$SRC_DIR" "$DEST_HOST:$DEST_DIR"

echo "Transfer complete."