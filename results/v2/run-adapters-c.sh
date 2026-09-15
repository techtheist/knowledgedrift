#!/bin/bash
cd /Users/techtheist/IdeaProjects/knowledgedrift
run() { # adapter dir world
  echo "=== $1 $3 start $(date -u +%FT%TZ)"
  adapters/.venv-$1/bin/python3 adapters/run.py --adapter $1 \
     --script worlds/v2/knowledgedrift-$3-v2.json --out adapters/out/$1-$3-v2.json 2>&1 | grep -v "│" | tail -2
  echo "=== $1 $3 end $(date -u +%FT%TZ)"
  cargo run --release -q -- --grade adapters/out/$1-$3-v2.json --script worlds/v2/knowledgedrift-$3-v2.json --json results/v2/$2/$3.json 2>&1 | grep -v "^\s*$" | tee results/v2/$2/$3.log | grep "v2 score\|success "
}
for w in 500-seed1 1500-seed1; do run langmem langmem-0.0.30 $w; done
for w in 500-seed1 1500-seed1; do run mem0 mem0-2.0.20 $w; done
echo "=== CHAIN C DONE"
