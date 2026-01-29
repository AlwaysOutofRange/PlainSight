#!/usr/bin/env bash
set -euo pipefail

SRC_DIR="/home/outofrange/Projects/PlainSight/"
DEST_HOST="god@cube"
DEST_DIR="Projects/PlainSight"

# Transfer
rsync -avz --progress "$SRC_DIR" "$DEST_HOST:$DEST_DIR"

# Delete specific folder on destination after successful transfer
ssh "$DEST_HOST" "rm -rf \"$DEST_DIR/copy_to_pi.sh\""

echo "Transfer complete."
