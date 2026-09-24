#!/bin/bash
# Engram Alpha 0.9.9 (engram-core ffbcd1a), engram arm only, judge Laya int4 (--nli-dir)
# instead of tasksource. Receipts beside the default-judge run.
cd "$(dirname "$0")/../../../.."
D=results/v2/reference-arms/engram-0.9.9-laya-int4
NLI=${NLI:-$HOME/.cache/engram/laya-en-int4}  # model.onnx = laya-onnx/en/model_int4.onnx (sha256 91df532b…)
for spec in "500 1" "500 2" "500 3"; do
  set -- $spec
  echo "=== $1 seed $2 start $(date -u +%FT%TZ)"
  target/release/knowledgedrift --v2 --sizes $1 --seed $2 --arms engram --nli-dir "$NLI" --json $D/$1-seed$2.json > $D/$1-seed$2.log 2>&1
  echo "=== $1 seed $2 end $(date -u +%FT%TZ) exit $?"
done
