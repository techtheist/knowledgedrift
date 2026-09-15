"""KnowledgeDrift adapter for cognee (the open-source ``cognee`` package).

cognee sells an LLM-built knowledge graph: ``cognify()`` classifies and
chunks every added document, then has a language model extract entities,
relationships and summaries into a graph that its completion searches walk.
The benchmark is offline and judge-free, so that half is out of scope. What
is measured here is the layer underneath it, which is also what cognee's
own ``SearchType.CHUNKS`` reads: documents added through ``cognee.add``,
processed by cognee's own ingestion pipeline with the LLM extraction task
left out (classify → chunk → ``add_data_points``), embedded by cognee's
``fastembed`` provider with the benchmark's reference model
(``BAAI/bge-small-en-v1.5``, 384 dims), stored in cognee's default LanceDB
vector store and LadybugDB graph store, and searched with
``cognee.search(query_type=SearchType.CHUNKS)`` — the one ranked,
LLM-free retrieval type cognee ships for text. See ``cognee.md`` for the
mapping table, every deviation, and the graded numbers.

No network, ever: telemetry and tracing are off, the mandatory LLM client
points at an unreachable loopback port and is never invoked, Hugging Face
is forced offline, and a socket guard turns any outbound TCP connect into an
exception (cognee's database workers talk over Unix pipes, which the guard
lets through).
"""

from __future__ import annotations

import asyncio
import os
import shutil
import socket
import uuid
from pathlib import Path
from typing import Any, Optional

HERE = Path(__file__).resolve().parent
OUT = HERE / "out"
STORE = Path(os.environ.get("KD_COGNEE_STORE", OUT / "cognee-store")).resolve()  # cognee refuses relative roots

# Must precede ``import cognee``: its settings objects read the environment
# at import time and cache themselves.
os.environ.setdefault("TELEMETRY_DISABLED", "1")
os.environ.setdefault("COGNEE_TRACING_ENABLED", "false")
os.environ.setdefault("COGNEE_SKIP_CONNECTION_TEST", "true")  # the first-run LLM ping would be the one LLM call
os.environ.setdefault("CACHING", "false")  # session cache + its LLM turn analysis on every search
os.environ.setdefault("AUTO_FEEDBACK", "false")  # LLM feedback detection per search turn
os.environ.setdefault("ENABLE_BACKEND_ACCESS_CONTROL", "false")  # one store, not one per dataset
os.environ.setdefault("KUZU_MAX_DB_SIZE", str(1 << 38))  # LadybugDB's buffer-manager cap (virtual, power of 2); the 1500 world exhausts the 32 GB default
os.environ.setdefault("HF_HUB_OFFLINE", "1")
os.environ.setdefault("TRANSFORMERS_OFFLINE", "1")
os.environ.setdefault("TOKENIZERS_PARALLELISM", "false")
os.environ.setdefault("LLM_PROVIDER", "openai")
os.environ.setdefault("LLM_MODEL", "never-called")
os.environ.setdefault("LLM_API_KEY", "sk-knowledgedrift-no-llm")
os.environ.setdefault("LLM_ENDPOINT", "http://127.0.0.1:9/v1")
os.environ.setdefault("EMBEDDING_PROVIDER", "fastembed")
os.environ.setdefault("EMBEDDING_MODEL", "BAAI/bge-small-en-v1.5")
os.environ.setdefault("EMBEDDING_DIMENSIONS", "384")
os.environ.setdefault("DATA_ROOT_DIRECTORY", str(STORE / "data"))
os.environ.setdefault("SYSTEM_ROOT_DIRECTORY", str(STORE / "system"))
os.environ.setdefault("CACHE_ROOT_DIRECTORY", str(STORE / "cache"))
os.environ.setdefault("COGNEE_LOGS_DIR", str(STORE / "logs"))
os.environ.setdefault("LOG_LEVEL", "WARNING")
os.environ.setdefault("COGNEE_LOG_FILE", "false")

import cognee  # noqa: E402
from cognee import SearchType  # noqa: E402
from cognee.modules.chunking.TextChunker import TextChunker  # noqa: E402
from cognee.modules.pipelines import run_pipeline  # noqa: E402
from cognee.modules.pipelines.layers.pipeline_execution_mode import get_pipeline_executor  # noqa: E402
from cognee.modules.pipelines.tasks.task import Task  # noqa: E402
from cognee.tasks.documents import classify_documents, extract_chunks_from_documents  # noqa: E402
from cognee.tasks.ingestion.data_item import DataItem  # noqa: E402
from cognee.tasks.storage import add_data_points  # noqa: E402

from adapter import Hit, Inscribed, MemoryAdapter, Recalled  # noqa: E402

DATASET = "knowledgedrift"
MAX_CHUNK_TOKENS = 512  # bge-small's window; every note in a v1 world is one chunk
KEY_NS = uuid.UUID("7c6f8a0e-3b1d-4c5e-9f2a-1d0e8b7a6c5f")


class NetworkAttempted(RuntimeError):
    """Raised by the socket guard: something tried to open a TCP connection."""


def _install_socket_guard() -> None:
    """Every outbound connect over an internet family raises. Unix-domain
    sockets stay open: cognee's LanceDB/LadybugDB workers are local
    ``multiprocessing`` children and their pipes never leave the machine."""

    real_connect = socket.socket.connect
    real_connect_ex = socket.socket.connect_ex

    def refuse(self, address, *args, **kwargs):  # noqa: ANN001
        if self.family == socket.AF_UNIX:
            return real_connect(self, address, *args, **kwargs)
        raise NetworkAttempted(f"cognee adapter: outbound connection attempted to {address!r}")

    def refuse_ex(self, address, *args, **kwargs):  # noqa: ANN001
        if self.family == socket.AF_UNIX:
            return real_connect_ex(self, address, *args, **kwargs)
        raise NetworkAttempted(f"cognee adapter: outbound connection attempted to {address!r}")

    socket.socket.connect = refuse  # type: ignore[method-assign]
    socket.socket.connect_ex = refuse_ex  # type: ignore[method-assign]
    socket.create_connection = lambda address, *a, **k: refuse(socket.socket(), address)  # type: ignore[assignment]


def _data_id(key: str) -> uuid.UUID:
    """The adapter picks cognee's data id itself (``DataItem.data_id``), so
    a key resolves without a lookup and a re-inscribed key gets a fresh id."""
    return uuid.uuid5(KEY_NS, key)


class Adapter(MemoryAdapter):
    name = "cognee"

    def __init__(self, store: Optional[str] = None) -> None:
        if store is not None and Path(store) != STORE:
            raise ValueError("cognee adapter: set KD_COGNEE_STORE in the environment instead of store=")
        self.store_dir = STORE
        self.loop = asyncio.new_event_loop()
        self.ids: dict[str, uuid.UUID] = {}  # key -> cognee data id
        self.keys: dict[uuid.UUID, str] = {}  # data id -> key
        self.dataset_id: Optional[uuid.UUID] = None
        self.dirty = False  # added since the last index pass
        self.index_passes = 0
        _install_socket_guard()

    def _run(self, coro):  # noqa: ANN001
        return self.loop.run_until_complete(coro)

    # ---- lifecycle --------------------------------------------------------

    def reset(self) -> None:
        shutil.rmtree(self.store_dir, ignore_errors=True)
        self.store_dir.mkdir(parents=True, exist_ok=True)
        cognee.config.data_root_directory(str(self.store_dir / "data"))
        cognee.config.system_root_directory(str(self.store_dir / "system"))
        self._run(cognee.prune.prune_data())
        self._run(cognee.prune.prune_system(metadata=True))
        self.ids.clear()
        self.keys.clear()
        self.dataset_id = None
        self.dirty = False

    def close(self) -> None:
        try:
            self._run(asyncio.sleep(0))
        finally:
            self.loop.close()

    def capabilities(self) -> dict[str, bool]:
        return {
            "link": False,  # edges come from the LLM extraction, which is not run
            "history": False,  # supersede = delete + add; nothing links the generations
            "trace": False,  # delete_data leaves nothing findable
            "suspects": False,  # contradiction detection is an LLM task (off by default too)
            "temporal": False,  # CHUNKS search has no time filter; the window is ignored
            "verdict": False,  # search never declines
            "write_check": False,  # add() stores silently
            "endorse_retrieval": False,
            "endorse_assistant": False,
            "endorse_user": False,
            "endorse_supervisor": False,
        }

    # ---- indexing ---------------------------------------------------------

    async def _index(self) -> None:
        """cognee's cognify pipeline minus the LLM task: classify the new
        Data rows into Documents, chunk them, persist chunks (+ their
        Document and NodeSet nodes) to the graph store and their text
        embeddings to LanceDB. ``incremental_loading`` makes cognee skip
        every Data row this pipeline already processed, so a pass costs
        what the new notes cost."""
        tasks = [
            Task(classify_documents),
            Task(extract_chunks_from_documents, max_chunk_size=MAX_CHUNK_TOKENS, chunker=TextChunker),
            Task(add_data_points, embed_triplets=False, task_config={"batch_size": 200}),
        ]
        executor = get_pipeline_executor(run_in_background=False)
        result = await executor(
            pipeline=run_pipeline,
            datasets=[DATASET],
            tasks=tasks,
            pipeline_name="cognify_pipeline",
            incremental_loading=True,
            use_pipeline_cache=False,
            data_per_batch=200,
        )
        for info in result.values() if isinstance(result, dict) else [result]:
            name = type(info).__name__
            if "Errored" in name:
                raise RuntimeError(f"cognee adapter: index pass errored: {info!r}")
        self.index_passes += 1
        self.dirty = False

    def _ensure_indexed(self) -> None:
        if self.dirty:
            self._run(self._index())

    # ---- writes -----------------------------------------------------------

    @staticmethod
    def _text(record: dict[str, Any]) -> str:
        return f"{record['title']}\n{record['body']}"

    def _remember(self, key: str, data_id: uuid.UUID) -> None:
        self.ids[key] = data_id
        self.keys[data_id] = key

    def _forget(self, key: str) -> uuid.UUID:
        data_id = self.ids.pop(key)  # KeyError on an unknown key: let it raise
        self.keys.pop(data_id, None)
        return data_id

    async def _add(self, key: str, record: dict[str, Any]) -> None:
        data_id = _data_id(key)
        if data_id in self.keys:
            raise ValueError(f"cognee adapter: data id for {key!r} is still live")
        item = DataItem(
            data=self._text(record),
            label=key,
            external_metadata={
                "key": key,
                "kind": record["kind"],
                "created_at": int(record["created_at"]),
                "code_refs": list(record.get("code_refs") or []),
            },
            data_id=data_id,
        )
        info = await cognee.add(item, dataset_name=DATASET, node_set=[key])
        if "Errored" in type(info).__name__:
            raise RuntimeError(f"cognee adapter: add() errored for {key!r}: {info!r}")
        if self.dataset_id is None:
            self.dataset_id = uuid.UUID(str(info.dataset_id))
        self._remember(key, data_id)
        self.dirty = True

    async def _delete(self, key: str) -> None:
        data_id = self._forget(key)
        assert self.dataset_id is not None
        await cognee.datasets.delete_data(dataset_id=self.dataset_id, data_id=data_id, mode="soft")

    def inscribe(self, record: dict[str, Any], mode: str) -> Inscribed:
        key = record["key"]
        if key in self.ids:
            raise ValueError(f"cognee adapter: key {key!r} is already live")
        self._run(self._add(key, record))
        return Inscribed()

    def supersede(self, old_key: str, new_record: dict[str, Any]) -> None:
        self._run(self._delete(old_key))
        self._run(self._add(new_record["key"], new_record))

    def release(self, key: str, reason: str) -> None:
        self._run(self._delete(key))

    def purge(self, key: str) -> None:
        self._run(self._delete(key))

    def settle(self) -> Optional[str]:
        self._ensure_indexed()
        return None

    # ---- reads ------------------------------------------------------------

    async def _search(self, query: str, k: int) -> list[Hit]:
        results = await cognee.search(
            query_text=query,
            query_type=SearchType.CHUNKS,
            datasets=[DATASET],
            top_k=k,
            only_context=True,  # skips the session-turn preparation (an LLM call when caching is on)
            verbose=True,  # keeps the ScoredResult objects, whose score CHUNKS otherwise drops
        )
        hits: list[Hit] = []
        for payload in results:
            for obj in payload.get("objects_result") or []:
                p = obj.payload or {}
                sets = p.get("belongs_to_set") or []
                keys = [s for s in sets if s in self.ids]
                key = keys[0] if len(keys) == 1 else None
                if key is not None and self.ids[key] != uuid.UUID(str(p.get("document_id") or self.ids[key])):
                    raise RuntimeError(f"cognee adapter: key {key!r} does not match the hit's document")
                hits.append(Hit(key=key, text=p["text"], score=-float(obj.score)))
        return hits

    def recall(self, query: str, k: int, window: Optional[dict[str, int]]) -> Recalled:
        self._ensure_indexed()
        if not self.ids:
            return Recalled([])
        return Recalled(self._run(self._search(query, k)))
