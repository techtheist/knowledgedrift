#!/bin/bash
# Engram Alpha 0.9.9 (engram-core ffbcd1a), engram arm only, default judge
# (tasksource). The baselines do not depend on the engram release and keep
# their 0.9.6-era receipts one directory up.
cd "$(dirname "$0")/../../../.."
D=results/v2/reference-arms/engram-0.9.9
for spec in "500 1" "500 2" "500 3" "1500 1"; do
  set -- $spec
  echo "=== $1 seed $2 start $(date -u +%FT%TZ)"
  target/release/knowledgedrift --v2 --sizes $1 --seed $2 --arms engram --json $D/$1-seed$2.json > $D/$1-seed$2.log 2>&1
  echo "=== $1 seed $2 end $(date -u +%FT%TZ) exit $?"
done
