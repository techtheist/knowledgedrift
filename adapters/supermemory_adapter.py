"""KnowledgeDrift adapter for Supermemory local (``supermemory-server``).

Runs the self-hosted single binary as a raw, LLM-free memory store: notes go
in through ``POST /v4/memories`` (the documented direct route — "memories are
embedded and immediately searchable", no ingestion pipeline), re-decisions
through the versioned ``PATCH /v4/memories`` (the old version is kept with
``isLatest=false`` under an ``updates`` relation), releases through the soft
``DELETE /v4/memories`` with its ``reason``, purges through a hard delete of
the memory's source document, reads through ``POST /v4/search`` with the
server's own defaults and a native numeric filter on a stored capture time.
The local embedder is switched to ``Xenova/bge-small-en-v1.5`` (384 dims),
the benchmark's reference model. See ``supermemory.md`` for the mapping
table, every deviation, and the graded numbers.

No LLM, verified: the server refuses to boot without a provider key, so it
gets a dummy one whose base URL is a tripwire — an HTTP listener inside this
process that records every request and answers 503. Any hit fails the run.
(The document route, ``POST /v3/documents``, trips it within seconds; the
direct-memory route never does.) The server runs with a scrubbed
environment and its own ``HOME``, so nothing in the caller's home is read
or written.
"""

from __future__ import annotations

import http.client
import json
import os
import shutil
import signal
import subprocess
import threading
import time
from datetime import datetime, timezone
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from typing import Any, Optional

from adapter import Hit, Inscribed, MemoryAdapter, Recalled

HERE = Path(__file__).resolve().parent
OUT = HERE / "out"
BINARY = HERE / "vendor" / "supermemory" / "bin" / "supermemory-server"

EMBED_MODEL = "Xenova/bge-small-en-v1.5"
EMBED_DIMS = 384
TAG = "knowledgedrift"


class LlmAttempted(RuntimeError):
    """Raised when the server sent anything to its configured LLM endpoint."""


class _Tripwire:
    """The server's "LLM provider": records every request, serves none."""

    def __init__(self) -> None:
        self.hits: list[str] = []
        hits = self.hits

        class Handler(BaseHTTPRequestHandler):
            def _hit(self) -> None:
                n = int(self.headers.get("content-length") or 0)
                if n:
                    self.rfile.read(n)
                hits.append(f"{self.command} {self.path}")
                self.send_response(503)
                self.end_headers()
                self.wfile.write(b'{"error":"knowledgedrift: no LLM in this run"}')

            do_GET = do_POST = do_PUT = do_PATCH = do_DELETE = _hit

            def log_message(self, *args: Any) -> None:  # noqa: ANN401
                pass

        self.server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        self.port = self.server.server_address[1]
        threading.Thread(target=self.server.serve_forever, daemon=True).start()

    def close(self) -> None:
        self.server.shutdown()
        self.server.server_close()


def _iso(ts: int) -> str:
    return datetime.fromtimestamp(int(ts), tz=timezone.utc).isoformat()


class Adapter(MemoryAdapter):
    name = "supermemory"

    def __init__(
        self,
        store: Optional[str] = None,
        port: str = "6795",
        search_mode: Optional[str] = None,
        threshold: Optional[str] = None,
        binary: Optional[str] = None,
    ) -> None:
        self.store_dir = Path(store).resolve() if store else OUT / "supermemory-store"
        self.models_dir = OUT / "supermemory-models"  # survives reset(): the embedder weights
        self.port = int(port)
        # None = the field is not sent and the server's default applies
        # (searchMode "memories", threshold 0.6 on 0.0.8).
        self.search_mode = search_mode
        self.threshold = float(threshold) if threshold is not None else None
        self.binary = Path(binary) if binary else BINARY
        self.proc: Optional[subprocess.Popen] = None
        self.log = None
        self.tripwire: Optional[_Tripwire] = None
        self.conn: Optional[http.client.HTTPConnection] = None
        self.read_retries = 0  # reads re-sent after a dropped connection (receipt note)
        self.boot_stalls = 0  # boots that never answered and were redone (receipt note)
        self.ids: dict[str, str] = {}  # key -> memory id (live notes)
        self.keys: dict[str, str] = {}  # memory id -> key (retired versions stay: the server keeps them)
        self.docs: dict[str, str] = {}  # key -> source document id
        self.texts: dict[str, str] = {}  # key -> stored text (the lineage read queries with it)

    # ---- lifecycle --------------------------------------------------------

    def _stop(self) -> None:
        if self.conn is not None:
            self.conn.close()
            self.conn = None
        if self.proc is not None:
            # The server starts a workflow engine (rivet) as a child: stop the group.
            # The group, not the parent, is what must be gone: a rivet still
            # shutting down hangs the next server at storage init.
            pgid = self.proc.pid
            deadline = time.monotonic() + 20
            sig = signal.SIGTERM
            while True:
                try:
                    os.killpg(pgid, sig)
                except (ProcessLookupError, PermissionError):
                    break  # empty — or, on macOS, only exiting members left (EPERM)
                # One TERM, then probe with signal 0; past the deadline, KILL.
                sig = signal.SIGKILL if time.monotonic() > deadline else 0
                self.proc.poll()  # reap the parent so the group can empty
                time.sleep(0.2)
            self.proc.wait()
            self.proc = None
            # EPERM is not proof: wait until no process still names this store.
            while subprocess.run(["pgrep", "-f", str(self.store_dir / "data")], capture_output=True).returncode == 0:
                if time.monotonic() > deadline + 20:
                    raise RuntimeError(f"supermemory adapter: a server process on {self.store_dir} would not stop")
                time.sleep(0.2)
        if self.log is not None:
            self.log.close()
            self.log = None
        if self.tripwire is not None:
            self.tripwire.close()
            self.tripwire = None

    def reset(self) -> None:
        if not self.binary.exists():
            raise RuntimeError(f"supermemory adapter: no binary at {self.binary} — see supermemory.md, Setup")
        # A boot now and then stalls at storage init, before the port opens
        # (seen only right after a heavy replay; the server logs nothing). The
        # store is empty either way, so a stalled boot is stopped, its log
        # kept, and a fresh one started — counted in the receipt note.
        self.boot_stalls = 0
        while not self._boot():
            self.boot_stalls += 1
            kept = OUT / f"supermemory-boot-stalled-{int(time.time())}.log"
            shutil.copyfile(self.store_dir / "server.log", kept)
            if self.boot_stalls == 3:
                self._stop()
                raise RuntimeError(f"supermemory adapter: the server stalled at boot three times — see {kept}")
        self.ids.clear()
        self.keys.clear()
        self.docs.clear()
        self.texts.clear()
        self.read_retries = 0
        status, listed = self._call("POST", "/v4/memories/list", {"containerTags": [TAG], "limit": 1}, ok=(200, 404))
        if status == 200 and listed.get("memoryEntries"):
            raise RuntimeError("supermemory adapter: reset() left memories in the container")

    def _boot(self) -> bool:
        """A fresh server on a fresh data directory; False if it never answered."""
        self._stop()
        shutil.rmtree(self.store_dir, ignore_errors=True)
        data = self.store_dir / "data"
        home = self.store_dir / "home"
        data.mkdir(parents=True)
        home.mkdir()
        self.models_dir.mkdir(parents=True, exist_ok=True)
        (data / "models").symlink_to(self.models_dir, target_is_directory=True)
        self.tripwire = _Tripwire()
        env = {
            "HOME": str(home),
            "PATH": "/usr/bin:/bin:/usr/sbin:/sbin",
            "PORT": str(self.port),
            "SUPERMEMORY_DATA_DIR": str(data),
            "SUPERMEMORY_EMBEDDING_PROVIDER": "local",
            "SUPERMEMORY_EMBEDDING_MODEL": EMBED_MODEL,
            "SUPERMEMORY_EMBEDDING_DIMENSIONS": str(EMBED_DIMS),
            "SUPERMEMORY_DISABLE_TELEMETRY": "1",
            "SUPERMEMORY_NO_UPDATE_CHECK": "1",
            "SUPERMEMORY_NO_OPEN": "1",
            "SUPERMEMORY_NO_STARTUP_ANIMATION": "1",
            # The boot check wants a provider key. This one leads to the
            # tripwire, and a hit there fails the run.
            "OPENAI_API_KEY": "sk-knowledgedrift-no-llm",
            "OPENAI_BASE_URL": f"http://127.0.0.1:{self.tripwire.port}/v1",
            "OPENAI_MODEL": "never-called",
        }
        self.log = open(self.store_dir / "server.log", "w")
        self.proc = subprocess.Popen(
            [str(self.binary)], env=env, cwd=str(self.store_dir),
            stdin=subprocess.DEVNULL, stdout=self.log, stderr=subprocess.STDOUT,
            start_new_session=True,
        )
        deadline = time.monotonic() + 60  # a healthy boot answers in about five seconds
        while time.monotonic() < deadline:
            if self.proc.poll() is not None:
                raise RuntimeError(f"supermemory adapter: server exited at boot — see {self.store_dir / 'server.log'}")
            if self._ready():
                return True
            time.sleep(0.25)
        return False

    def _ready(self) -> bool:
        # Its own short-lived connection: the port opens before the store is
        # ready (503 "Service is starting"), and a stalled boot may never answer.
        conn = http.client.HTTPConnection("127.0.0.1", self.port, timeout=5)
        try:
            conn.request("POST", "/v4/memories/list", body=json.dumps({"containerTags": [TAG], "limit": 1}),
                         headers={"content-type": "application/json"})
            resp = conn.getresponse()
            resp.read()
            return resp.status in (200, 404)
        except (ConnectionError, http.client.HTTPException, OSError):
            return False
        finally:
            conn.close()

    def close(self) -> None:
        hits = list(self.tripwire.hits) if self.tripwire else []
        self._stop()
        if hits:
            raise LlmAttempted(f"supermemory adapter: the server called its LLM endpoint {len(hits)}×: {hits[:3]}")

    def capabilities(self) -> dict[str, bool]:
        return {
            "link": False,  # relations (updates/extends/derives) are minted by the LLM pipeline; no API stores a caller's edge
            "history": True,  # PATCH keeps the old version under an `updates` relation; search reads the ancestry back
            "trace": False,  # a forgotten memory keeps its forgetReason in the store, but nothing on 0.0.8 delivers it
            "suspects": False,  # no conflict queue without the LLM
            "temporal": True,  # native numeric metadata filter inside /v4/search
            "verdict": False,  # a search under the threshold comes back empty; there is no decline signal beside hits
            "write_check": False,  # POST /v4/memories stores a verbatim duplicate silently
        }

    # ---- transport --------------------------------------------------------

    def _call(self, method: str, path: str, body: Optional[dict[str, Any]] = None, ok: tuple[int, ...] = (200, 201)) -> tuple[int, Any]:
        if self.tripwire is not None and self.tripwire.hits:
            raise LlmAttempted(f"supermemory adapter: the server called its LLM endpoint: {self.tripwire.hits[:3]}")
        payload = json.dumps(body).encode() if body is not None else None
        # One keep-alive connection: a connection per request had the server
        # reset one some 4,000 requests into the 1500 world. A read is
        # idempotent and is retried on a fresh connection; a write never is —
        # a blind retry could store a note twice, so it fails the run instead.
        attempts = 3 if path in ("/v4/search", "/v4/memories/list") else 1
        for attempt in range(attempts):
            if self.conn is None:
                self.conn = http.client.HTTPConnection("127.0.0.1", self.port, timeout=300)
            try:
                # Unauthenticated localhost requests get the server's own key.
                self.conn.request(method, path, body=payload, headers={"content-type": "application/json"} if payload else {})
                resp = self.conn.getresponse()
                raw = resp.read().decode("utf-8", "replace")
                break
            except (ConnectionError, http.client.HTTPException, OSError):
                self.conn.close()
                self.conn = None
                if attempt + 1 == attempts:
                    raise
                self.read_retries += 1
        if resp.status not in ok:
            raise RuntimeError(f"supermemory adapter: {method} {path} -> {resp.status}: {raw[:400]}")
        if not raw.lstrip().startswith(("{", "[")):
            return resp.status, None  # 204, or the plain-text 503 while booting
        return resp.status, json.loads(raw)

    # ---- writes -----------------------------------------------------------

    @staticmethod
    def _text(record: dict[str, Any]) -> str:
        return f"{record['title']}\n{record['body']}"

    @staticmethod
    def _metadata(record: dict[str, Any]) -> dict[str, Any]:
        return {
            "key": record["key"],
            "kind": record["kind"],
            "created_unix": int(record["created_at"]),  # what the window filter reads
        }

    def inscribe(self, record: dict[str, Any], mode: str) -> Inscribed:
        key = record["key"]
        if key in self.ids:
            raise ValueError(f"supermemory adapter: key {key!r} is already live")
        text = self._text(record)
        _, res = self._call("POST", "/v4/memories", {
            "containerTag": TAG,
            "memories": [{
                "content": text,
                "metadata": self._metadata(record),
                # The memory's own date slot; createdAt is the server's wall clock and not settable.
                "temporalContext": {"documentDate": _iso(record["created_at"])},
            }],
        })
        made = res["memories"]
        if len(made) != 1 or made[0]["memory"] != text or not res.get("documentId"):
            raise RuntimeError(f"supermemory adapter: unexpected create result for {key!r}: {res!r}")
        self.ids[key] = made[0]["id"]
        self.keys[made[0]["id"]] = key
        self.docs[key] = res["documentId"]
        self.texts[key] = text
        return Inscribed()

    def supersede(self, old_key: str, new_record: dict[str, Any]) -> None:
        old_id = self.ids.pop(old_key)  # KeyError on an unknown key: let it raise
        self.texts.pop(old_key, None)
        text = self._text(new_record)
        _, res = self._call("PATCH", "/v4/memories", {
            "id": old_id,
            "containerTag": TAG,
            "newContent": text,
            "metadata": self._metadata(new_record),
        })
        if res.get("parentMemoryId") != old_id or res["memory"] != text:
            raise RuntimeError(f"supermemory adapter: PATCH did not version {old_key!r}: {res!r}")
        key = new_record["key"]
        self.ids[key] = res["id"]
        self.keys[res["id"]] = key  # old_id -> old_key stays: a delivered retired version must be named
        self.texts[key] = text

    def release(self, key: str, reason: str) -> None:
        memory_id = self.ids.pop(key)
        self.texts.pop(key, None)
        _, res = self._call("DELETE", "/v4/memories", {"id": memory_id, "containerTag": TAG, "reason": reason})
        if not res.get("forgotten"):
            raise RuntimeError(f"supermemory adapter: forget of {key!r} answered {res!r}")

    def purge(self, key: str) -> None:
        self.ids.pop(key)
        self.texts.pop(key, None)
        # One note per create call, so the source document holds this memory
        # alone; deleting it removes the memory row, not just its visibility.
        self._call("DELETE", f"/v3/documents/{self.docs.pop(key)}", ok=(200, 204))

    def settle(self) -> Optional[str]:
        version = self.binary.with_name(self.binary.name + ".version")
        return (
            f"supermemory-server {version.read_text().strip() if version.exists() else '?'}, "
            f"searchMode={self.search_mode or 'default'}, threshold={self.threshold if self.threshold is not None else 'default'}, "
            f"LLM tripwire hits {len(self.tripwire.hits) if self.tripwire else '?'}, "
            f"reads re-sent after a dropped connection {self.read_retries}, "
            f"stalled boots redone {self.boot_stalls}"
        )

    # ---- reads ------------------------------------------------------------

    def _search(self, body: dict[str, Any]) -> list[dict[str, Any]]:
        body = {"containerTag": TAG, **body}
        if self.search_mode is not None:
            body.setdefault("searchMode", self.search_mode)
        if self.threshold is not None:
            body.setdefault("threshold", self.threshold)
        _, res = self._call("POST", "/v4/search", body)
        return res["results"]

    def recall(self, query: str, k: int, window: Optional[dict[str, int]]) -> Recalled:
        body: dict[str, Any] = {"q": query, "limit": k}
        if window:
            clauses = []
            if window.get("after") is not None:
                clauses.append({"filterType": "numeric", "key": "created_unix", "value": str(int(window["after"])), "numericOperator": ">="})
            if window.get("before") is not None:
                clauses.append({"filterType": "numeric", "key": "created_unix", "value": str(int(window["before"])), "numericOperator": "<"})
            if clauses:
                body["filters"] = {"AND": clauses}
        hits: list[Hit] = []
        for r in self._search(body):
            meta = r.get("metadata") or {}
            key = self.keys.get(r["id"])
            if key is not None and meta.get("key") != key:
                raise RuntimeError(f"supermemory adapter: id map says {key!r}, metadata says {meta.get('key')!r}")
            created = meta.get("created_unix")
            hits.append(Hit(
                key=key,
                text=r.get("memory") or r.get("chunk") or "",
                score=r.get("similarity"),
                created_at=int(created) if created is not None else None,
            ))
        return Recalled(hits)

    def lineage(self, key: str) -> Optional[list[str]]:
        # The ancestry is the server's: the head found by its own stored key,
        # its `updates` parents read off include.relatedMemories. The query is
        # the head's own text so the filtered search cannot rank it away.
        results = self._search({
            "q": self.texts[key],
            "limit": 5,
            "threshold": 0,
            "filters": {"AND": [{"key": "key", "value": key}]},
            "include": {"relatedMemories": True},
        })
        head = next((r for r in results if r["id"] == self.ids[key]), None)
        if head is None:
            raise RuntimeError(f"supermemory adapter: lineage could not find the head {key!r}")
        parents = (head.get("context") or {}).get("parents") or []
        return [p["metadata"]["key"] for p in parents if p.get("relation") == "updates" and (p.get("metadata") or {}).get("key")]
