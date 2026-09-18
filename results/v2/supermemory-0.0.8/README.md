# Supermemory local 0.0.8 — receipts

The self-hosted `supermemory-server` binary with no LLM — direct memories
(`POST /v4/memories`), versioned updates, the server's search defaults,
its local embedder on bge-small (`adapters/supermemory.md`) — the adapter
as merged, replayed on `worlds/v2/knowledgedrift-{500,1500}-seed1-v2.json`
(2026-09-18, one Apple-silicon laptop, CPU, the chain running alone:
`results/v2/run-adapters-d.sh`). Files `500-seed1.json` and
`1500-seed1.json` are what `knowledgedrift --grade … --json` wrote; the
`.log` beside each is the terminal report. `*-threshold0.*` are the
ablation with the search's similarity floor switched off
(`--kw threshold=0`) — not the row. Every transcript's `settle_note`
reads "LLM tripwire hits 0". The rows are in `results/v2/README.md`.
