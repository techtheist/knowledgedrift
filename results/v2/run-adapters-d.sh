#!/bin/bash
cd /Users/techtheist/IdeaProjects/knowledgedrift
run() { # world suffix adapter-kwargs...
  w=$1; sfx=$2; shift 2
  out=adapters/out/supermemory-$w$sfx-v2.json
  rm -f $out   # a failed replay must leave nothing behind to grade
  echo "=== supermemory $w$sfx start $(date -u +%FT%TZ)"
  python3 adapters/run.py --adapter supermemory "$@" \
     --script worlds/v2/knowledgedrift-$w-v2.json --out $out 2>&1 | tail -2
  echo "=== supermemory $w$sfx end $(date -u +%FT%TZ)"
  [ -f $out ] || { echo "=== supermemory $w$sfx REPLAY FAILED — not graded"; return 1; }
  cargo run --release -q -- --grade $out --script worlds/v2/knowledgedrift-$w-v2.json --json results/v2/supermemory-0.0.8/$w$sfx.json 2>&1 | grep -v "^\s*$" | tee results/v2/supermemory-0.0.8/$w$sfx.log | grep "v2 score\|success "
}
# the row: the server's own search defaults (searchMode memories, threshold 0.6)
for w in 500-seed1 1500-seed1; do run $w ""; done
# the ablation: the similarity floor switched off
for w in 500-seed1 1500-seed1; do run $w -threshold0 --kw threshold=0; done
echo "=== CHAIN D DONE"
