"""KnowledgeDrift adapter for LangMem's memory layer.

LangMem (LangChain's ``langmem`` package) is an LLM-facing memory toolkit —
its memory managers and ``create_manage_memory_tool`` /
``create_search_memory_tool`` are prompts and tools an agent's LLM drives.
The offline, judge-free benchmark cannot run an LLM, so this adapter
exercises the layer those wrappers sit on: a LangGraph ``BaseStore``
(``langgraph.store.memory.InMemoryStore``) with LangMem's semantic index —
exactly the ``store.put`` / ``store.search`` / ``store.delete`` calls the
LangMem tools make on the agent's behalf. Nothing here is extracted,
consolidated or judged by a model; the embedder is the product's own
``BAAI/bge-small-en-v1.5`` (384 dims) through ``fastembed``, local-only.

See ``langmem.md`` for the mapping table, the declared capabilities and
every deviation.
"""

from __future__ import annotations

from typing import Any, Optional

from adapter import Hit, Inscribed, MemoryAdapter, Recalled

NAMESPACE = ("kd",)
EMBED_MODEL = "BAAI/bge-small-en-v1.5"
DIMS = 384


class Adapter(MemoryAdapter):
    name = "langmem"

    def __init__(self, model: str = EMBED_MODEL) -> None:
        # One embedder per process; the store is rebuilt on every reset().
        from fastembed import TextEmbedding

        self._embedder = TextEmbedding(model)
        self._store = None
        self._keys: set[str] = set()  # the script keys currently live in the store

    # ---- embedding --------------------------------------------------------

    def _embed(self, texts: list[str]) -> list[list[float]]:
        """The store's ``embed`` callable: texts -> vectors, batch-embedded."""
        vecs = [v.tolist() for v in self._embedder.embed(texts)]
        assert all(len(v) == DIMS for v in vecs), "embedder dims drifted"
        return vecs

    # ---- lifecycle --------------------------------------------------------

    def reset(self) -> None:
        from langgraph.store.memory import InMemoryStore

        self._store = InMemoryStore(index={"dims": DIMS, "embed": self._embed, "fields": ["text"]})
        self._keys = set()

    def close(self) -> None:
        self._store = None

    def capabilities(self) -> dict[str, bool]:
        return {
            "link": False,        # a flat key/value store; no edges
            "history": False,     # supersede is delete + put, nothing keeps the old one
            "trace": False,       # release/purge are plain deletes; no marker survives
            "suspects": False,    # no disagreement queue; no NLI, no LLM
            "temporal": True,     # capture time is stored and filtered natively at search time
            "verdict": False,     # put() is silent; no near-duplicate refusal
            "write_check": False, # no write-time canon check
        }

    # ---- writes -----------------------------------------------------------

    @staticmethod
    def _value(record: dict[str, Any]) -> dict[str, Any]:
        return {
            "text": f"{record['title']}\n{record['body']}",
            "kind": record["kind"],
            "created_at": int(record["created_at"]),
            "key": record["key"],
        }

    def _put(self, record: dict[str, Any]) -> None:
        self._store.put(NAMESPACE, record["key"], self._value(record))
        self._keys.add(record["key"])

    def _delete(self, key: str) -> None:
        # Missing keys are a script error the runner must see; the store's
        # own delete is a silent no-op, so check the map ourselves.
        if key not in self._keys:
            raise KeyError(f"{key!r} is not in the store")
        self._store.delete(NAMESPACE, key)
        self._keys.discard(key)

    def inscribe(self, record: dict[str, Any], mode: str) -> Inscribed:
        # "import" and "write" are the same call: the store has no
        # write-time checks, so a write returns the silent default verdict.
        self._put(record)
        return Inscribed()

    def supersede(self, old_key: str, new_record: dict[str, Any]) -> None:
        # Flat replacement: the retired generation is deleted, the head is
        # put. No lineage survives (history: false).
        self._delete(old_key)
        self._put(new_record)

    def release(self, key: str, reason: str) -> None:
        # The store has no deletion marker; the reason is dropped (trace: false).
        self._delete(key)

    def purge(self, key: str) -> None:
        self._delete(key)

    # ---- reads ------------------------------------------------------------

    def recall(self, query: str, k: int, window: Optional[dict[str, int]]) -> Recalled:
        filt = None
        if window is not None:
            # Half-open [after, before) on the stored capture time. The store
            # applies the filter to candidates BEFORE the top-k cut (verified
            # against the pinned version in langmem.md), so no over-fetch.
            filt = {"created_at": {"$gte": int(window["after"]), "$lt": int(window["before"])}}
        items = self._store.search(NAMESPACE, query=query, limit=k, filter=filt)
        hits = [
            Hit(
                key=item.key,
                text=item.value["text"],
                score=item.score,
                created_at=item.value["created_at"],
            )
            for item in items
        ]
        return Recalled(hits=hits)

    # link / suspects / lineage / settle / standing_tokens: base-class
    # defaults (False / None / None / None / 0) — the store has none of them.
