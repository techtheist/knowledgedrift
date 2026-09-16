#!/bin/bash

cd "$(dirname "$0")/../../.."

A=engram,rag,grep,curated,whole,chance,tfidf
for spec in "500 1" "500 2" "500 3" "1500 1"; do
  set -- $spec
  echo "=== v2 $1 seed $2 start $(date -u +%FT%TZ)"
  cargo run --release --features fastembed -- --v2 --sizes $1 --seed $2 --arms $A --json results/v2/reference-arms/$1-seed$2.json > results/v2/reference-arms/$1-seed$2.log 2>&1
  echo "=== v2 $1 seed $2 end $(date -u +%FT%TZ) exit $?"
done
echo "=== v1 500 seed 1 start $(date -u +%FT%TZ)"
cargo run --release --features fastembed -- --sizes 500 --seed 1 --arms engram --json results/v1/reference-arms/500-seed1-engram-0.9.6.json > results/v1/reference-arms/500-seed1-engram-0.9.6.log 2>&1
echo "=== v1 500 seed 1 end $(date -u +%FT%TZ) exit $?"
echo "=== LADDER DONE"
