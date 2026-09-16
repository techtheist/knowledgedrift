#!/bin/bash
cd /Users/techtheist/IdeaProjects/knowledgedrift
A=engram,rag,grep,curated,whole,chance,tfidf
for spec in "500 1" "500 2" "500 3" "1500 1"; do
  set -- $spec
  echo "=== $1 seed $2 start $(date -u +%FT%TZ)"
  cargo run --release --features fastembed -- --v2 --sizes $1 --seed $2 --arms $A --json results/v2/reference-arms/$1-seed$2.json > results/v2/reference-arms/$1-seed$2.log 2>&1
  echo "=== $1 seed $2 end $(date -u +%FT%TZ) exit $?"
done
echo "=== V2 LADDER DONE"
