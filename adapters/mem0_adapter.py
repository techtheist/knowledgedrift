"""KnowledgeDrift adapter for Mem0 (the open-source ``mem0ai`` package).

Runs Mem0 as a raw store: ``add(infer=False)`` (no LLM extraction), the
local ``huggingface`` embedder loading ``BAAI/bge-small-en-v1.5`` (the
product's model, 384 dims), Mem0's embedded Qdrant under ``out/`` with its
own hybrid dense + BM25 scoring, and native range filters on a stored
capture time. See ``mem0.md`` for the mapping table, every deviation, and
the graded numbers.

No network, ever: telemetry is off, the LLM client points at an unreachable
loopback port and is never invoked, Hugging Face is forced offline (models
must be prefetched once — see mem0.md), and a socket guard turns any
outbound connect into an exception so a slip cannot pass unnoticed.
"""

from __future__ import annotations

import os
import shutil
import socket
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Optional

HERE = Path(__file__).resolve().parent
OUT = HERE / "out"

# Must precede ``import mem0`` and any Hugging Face import: both read these
# at import time.
os.environ.setdefault("MEM0_TELEMETRY", "False")  # posthog, notices, remote config, migrations store
os.environ.setdefault("MEM0_DIR", str(OUT / "mem0-home"))  # keeps ~/.mem0 untouched
os.environ.setdefault("HF_HUB_OFFLINE", "1")
os.environ.setdefault("TRANSFORMERS_OFFLINE", "1")
os.environ.setdefault("TOKENIZERS_PARALLELISM", "false")

from mem0 import Memory  # noqa: E402

from adapter import Hit, Inscribed, MemoryAdapter, Recalled  # noqa: E402

EMBED_MODEL = "BAAI/bge-small-en-v1.5"
EMBED_DIMS = 384
USER_ID = "knowledgedrift"
COLLECTION = "knowledgedrift"


class NetworkAttempted(RuntimeError):
    """Raised by the socket guard: something tried to open a connection."""


def _install_socket_guard() -> None:
    """Every outbound connect raises. Local Qdrant, torch, onnxruntime and
    SQLite need no sockets; the only things that would are exactly the
    things this benchmark forbids (LLM, telemetry, model downloads)."""

    def refuse(self, address, *args, **kwargs):  # noqa: ANN001
        raise NetworkAttempted(f"mem0 adapter: outbound connection attempted to {address!r}")

    socket.socket.connect = refuse  # type: ignore[method-assign]
    socket.socket.connect_ex = refuse  # type: ignore[method-assign]
    socket.create_connection = lambda address, *a, **k: refuse(None, address)  # type: ignore[assignment]


def _iso(ts: int) -> str:
    return datetime.fromtimestamp(int(ts), tz=timezone.utc).isoformat()


class Adapter(MemoryAdapter):
    name = "mem0"

    def __init__(self, store: Optional[str] = None, device: str = "cpu") -> None:
        self.store_dir = Path(store) if store else OUT / "mem0-store"
        self.device = device
        self.mem: Optional[Memory] = None
        self.ids: dict[str, str] = {}  # key -> mem0 memory id
        self.keys: dict[str, str] = {}  # mem0 memory id -> key
        _install_socket_guard()

    # ---- lifecycle --------------------------------------------------------

    def _config(self) -> dict[str, Any]:
        return {
            "vector_store": {
                "provider": "qdrant",
                "config": {
                    "collection_name": COLLECTION,
                    "embedding_model_dims": EMBED_DIMS,
                    "path": str(self.store_dir / "qdrant"),
                    "on_disk": False,
                },
            },
            "embedder": {
                "provider": "huggingface",
                "config": {
                    "model": EMBED_MODEL,
                    "embedding_dims": EMBED_DIMS,
                    "model_kwargs": {"device": self.device},
                },
            },
            # Constructed because Memory() insists on one; never called with
            # infer=False. The base URL is a loopback port nothing listens on
            # and the socket guard would raise before a packet left anyway.
            "llm": {
                "provider": "openai",
                "config": {
                    "model": "never-called",
                    "api_key": "sk-knowledgedrift-no-llm",
                    "openai_base_url": "http://127.0.0.1:9/v1",
                },
            },
            "history_db_path": str(self.store_dir / "history.db"),
        }

    def _close_memory(self) -> None:
        if self.mem is None:
            return
        client = getattr(self.mem.vector_store, "client", None)
        if client is not None:
            client.close()  # releases the embedded Qdrant lock on the path
        self.mem.close()
        self.mem = None

    def reset(self) -> None:
        self._close_memory()
        shutil.rmtree(self.store_dir, ignore_errors=True)
        self.store_dir.mkdir(parents=True, exist_ok=True)
        self.mem = Memory.from_config(self._config())
        self.ids.clear()
        self.keys.clear()
        n = self.mem.vector_store.client.count(COLLECTION).count
        if n != 0:
            raise RuntimeError(f"mem0 adapter: reset() left {n} points in the collection")

    def close(self) -> None:
        self._close_memory()

    def capabilities(self) -> dict[str, bool]:
        return {
            "link": False,  # Mem0 has no edges (its graph memory needs an LLM)
            "history": False,  # update() keeps the id; history() is a change log, not a chain
            "trace": False,  # delete() leaves nothing findable
            "suspects": False,  # no conflict queue
            "temporal": True,  # native Range filter on the stored capture time
            "verdict": False,  # search never declines
            "write_check": False,  # add(infer=False) stores silently
        }

    # ---- writes -----------------------------------------------------------

    @staticmethod
    def _text(record: dict[str, Any]) -> str:
        return f"{record['title']}\n{record['body']}"

    @staticmethod
    def _metadata(record: dict[str, Any]) -> dict[str, Any]:
        ts = int(record["created_at"])
        return {
            "key": record["key"],
            "kind": record["kind"],
            "created_at": _iso(ts),  # Mem0's own created_at slot honours a caller value
            "created_unix": ts,  # numeric twin for the native range filter
        }

    def _remember(self, key: str, memory_id: str) -> None:
        self.ids[key] = memory_id
        self.keys[memory_id] = key

    def _forget(self, key: str) -> str:
        memory_id = self.ids.pop(key)  # KeyError on an unknown key: let it raise
        self.keys.pop(memory_id, None)
        return memory_id

    def inscribe(self, record: dict[str, Any], mode: str) -> Inscribed:
        assert self.mem is not None
        key = record["key"]
        if key in self.ids:
            raise ValueError(f"mem0 adapter: key {key!r} is already live")
        res = self.mem.add(self._text(record), user_id=USER_ID, metadata=self._metadata(record), infer=False)
        results = res["results"]
        if len(results) != 1 or results[0].get("event") != "ADD":
            raise RuntimeError(f"mem0 adapter: unexpected add() result for {key!r}: {res!r}")
        self._remember(key, results[0]["id"])
        return Inscribed()

    def supersede(self, old_key: str, new_record: dict[str, Any]) -> None:
        assert self.mem is not None
        memory_id = self._forget(old_key)
        self.mem.update(memory_id, text=self._text(new_record), metadata=self._metadata(new_record))
        self._remember(new_record["key"], memory_id)

    def release(self, key: str, reason: str) -> None:
        assert self.mem is not None
        self.mem.delete(self._forget(key))

    def purge(self, key: str) -> None:
        assert self.mem is not None
        self.mem.delete(self._forget(key))

    # ---- reads ------------------------------------------------------------

    def recall(self, query: str, k: int, window: Optional[dict[str, int]]) -> Recalled:
        assert self.mem is not None
        filters: dict[str, Any] = {"user_id": USER_ID}
        if window:
            bounds: dict[str, int] = {}
            if window.get("after") is not None:
                bounds["gte"] = int(window["after"])
            if window.get("before") is not None:
                bounds["lt"] = int(window["before"])
            if bounds:
                filters["created_unix"] = bounds
        res = self.mem.search(query, filters=filters, top_k=k)
        hits: list[Hit] = []
        for r in res["results"]:
            meta = r.get("metadata") or {}
            key = self.keys.get(r["id"])
            if key is not None and meta.get("key") != key:
                raise RuntimeError(f"mem0 adapter: id map says {key!r}, payload says {meta.get('key')!r}")
            created = meta.get("created_unix")
            hits.append(
                Hit(
                    key=key,
                    text=r["memory"],
                    score=r.get("score"),
                    created_at=int(created) if created is not None else None,
                )
            )
        return Recalled(hits)
