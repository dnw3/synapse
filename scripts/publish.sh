#!/usr/bin/env bash
#
# Publish all synaptic crates to crates.io in dependency order.
#
# Usage:
#   ./scripts/publish.sh          # publish all crates
#   ./scripts/publish.sh --dry-run # dry-run (no actual publish)
#
set -euo pipefail

DRY_RUN=""
if [[ "${1:-}" == "--dry-run" ]]; then
  DRY_RUN="--dry-run"
  echo "==> DRY RUN mode"
fi

# Topological publish order (dependencies before dependents)
CRATES=(
  synaptic-core
  synaptic-macros
  synaptic-models
  synaptic-callbacks
  synaptic-tools
  synaptic-runnables
  synaptic-store
  synaptic-middleware
  synaptic-mcp
  synaptic-embeddings
  synaptic-memory
  synaptic-parsers
  synaptic-retrieval
  synaptic-cache
  synaptic-eval
  synaptic-prompts
  synaptic-loaders
  synaptic-splitters
  synaptic-vectorstores
  synaptic-graph
  synaptic-deep
  synaptic
)

TOTAL=${#CRATES[@]}
IDX=0

for crate in "${CRATES[@]}"; do
  IDX=$((IDX + 1))
  echo ""
  echo "==> [$IDX/$TOTAL] Publishing $crate ..."
  cargo publish -p "$crate" $DRY_RUN --allow-dirty

  if [[ -z "$DRY_RUN" ]]; then
    # Wait for crates.io index to update before publishing dependents
    echo "    Waiting 30s for crates.io index..."
    sleep 30
  fi
done

echo ""
echo "==> All $TOTAL crates published successfully!"
