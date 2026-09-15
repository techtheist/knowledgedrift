#!/bin/bash
cd /Users/techtheist/IdeaProjects/knowledgedrift
for w in 500-seed1 1500-seed1; do
  echo "=== memcontinuum $w start $(date -u +%FT%TZ)"
  adapters/.venv-memcontinuum/bin/python3 adapters/run.py --adapter memcontinuum \
     --script worlds/v2/knowledgedrift-$w-v2.json --out adapters/out/memcontinuum-$w-v2.json 2>&1 | grep -v "│" | tail -2
  echo "=== memcontinuum $w end $(date -u +%FT%TZ)"
  cargo run --release -q -- --grade adapters/out/memcontinuum-$w-v2.json --script worlds/v2/knowledgedrift-$w-v2.json --json results/v2/memcontinuum-0.2.0rc5/$w.json 2>&1 | grep -v "^\s*$" | tee results/v2/memcontinuum-0.2.0rc5/$w.log | grep "v2 score\|success "
done
echo "=== CHAIN B DONE"
