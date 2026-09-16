"""KnowledgeDrift adapter for MemContinuum (github.com/krakozavr/MemContinuum).

MemContinuum is a decision-chain memory over hand-authored markdown: one
TOPIC file per subject, an append-only list of LINKS (rulings) under it,
each link carrying a status and a per-field authority (owner-verbatim,
owner-ratified, agent-inference, …); ``memidx.py`` indexes the store into
SQLite FTS5 plus bge-small vectors (fastembed) and answers ``search`` with
RRF-fused hybrid ranking over the active rulings. See ``memcontinuum.md``
for the mapping table, every deviation, and the graded numbers.

This adapter drives the engine in-process (``import memidx``): every
protocol write becomes a markdown topic file under the store root, the
index is rebuilt lazily before the next read (``cmd_reindex``, which only
re-embeds changed files), and every recall is ``cmd_search --json`` with
the system's own defaults (hybrid mode, active rulings only). The one
performance shim: the embedding model is loaded once and handed back from
``memidx.load_embedding_model`` instead of being re-instantiated per call
(the engine does that on purpose for its own test isolation; it changes
nothing about the vectors).

No LLM anywhere: the engine has none. The bge-small weights come from
fastembed's own registry (prefetched once — see memcontinuum.md).
"""

from __future__ import annotations

import argparse
import contextlib
import io
import json
import os
import shutil
import sqlite3
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Optional

import yaml

HERE = Path(__file__).resolve().parent
OUT = HERE / "out"
SRC = Path(os.environ.get("MEMCONTINUUM_SRC", str(HERE / "vendor" / "memcontinuum")))
if not (SRC / "memidx.py").exists():
    raise RuntimeError(
        f"memcontinuum adapter: memidx.py not found under {SRC}; clone "
        "https://github.com/krakozavr/MemContinuum there (or set MEMCONTINUUM_SRC)"
    )
sys.path.insert(0, str(SRC))
os.environ.setdefault("MEMCONTINUUM_HOME", str(OUT / "memcontinuum-home"))

import memidx  # noqa: E402

from adapter import Hit, Inscribed, MemoryAdapter, Recalled  # noqa: E402

PROJECT = "knowledgedrift"


def _day(ts: Optional[int]) -> str:
    if ts is None:
        return datetime.now(tz=timezone.utc).date().isoformat()
    return datetime.fromtimestamp(int(ts), tz=timezone.utc).date().isoformat()


def _day_unix(day: str) -> int:
    return int(datetime.fromisoformat(day).replace(tzinfo=timezone.utc).timestamp())


class Topic:
    """One topic file: frontmatter fields, its links, and which harness key
    each link answers to (a promotion link answers to the key it promotes)."""

    def __init__(self, tid: str, title: str, area: str, code_refs: list[str], tags: list[str]) -> None:
        self.id = tid
        self.title = title
        self.area = area
        self.code_refs = code_refs
        self.tags = tags
        self.links: list[dict[str, Any]] = []  # oldest first; written newest first
        self.key_of: dict[str, str] = {}  # link id -> key
        self.notes: list[str] = []  # free body lines (a release reason)

    def next_link(self) -> str:
        return f"L{len(self.links) + 1}"

    def current(self) -> Optional[dict[str, Any]]:
        for link in reversed(self.links):
            if link.get("status") == "active":
                return link
        return None

    def link_for_key(self, key: str) -> Optional[dict[str, Any]]:
        for link in reversed(self.links):
            if self.key_of.get(link["link"]) == key:
                return link
        return None

    def path(self, root: Path) -> Path:
        return root / "topics" / self.area / f"{self.id}.md"

    def render(self) -> str:
        current = self.current()
        fm: dict[str, Any] = {
            "type": "topic",
            "id": self.id,
            "title": self.title,
            "area": self.area,
            "project": PROJECT,
            "current": current["link"] if current else None,
            "code_refs": list(self.code_refs),
            "tags": list(self.tags),
            "links": list(reversed(self.links)),
        }
        head = yaml.safe_dump(fm, sort_keys=False, allow_unicode=True, width=1000)
        body_lines = [f"# {self.title}", ""]
        for link in reversed(self.links):
            ruling = link.get("ruling") or {}
            body_lines.append(
                f"- {link['link']} {link.get('date')} {link.get('kind')} ({link.get('status')}, "
                f"{ruling.get('authority')}): {ruling.get('text', '')}"
            )
        body_lines.extend(self.notes)
        return f"---\n{head}---\n\n" + "\n".join(body_lines) + "\n"


class Adapter(MemoryAdapter):
    name = "memcontinuum"

    def __init__(self, store: Optional[str] = None, db: Optional[str] = None, mode: str = "hybrid") -> None:
        self.root = Path(store) if store else OUT / "memcontinuum-store"
        self.db = Path(db) if db else OUT / "memcontinuum-index.sqlite"
        self.mode = mode
        self.topics: dict[str, Topic] = {}
        self.key_topic: dict[str, str] = {}  # key -> topic id
        self.dirty = False
        self.reindexes = 0
        self._model = None
        self._fp: Optional[str] = None
        # The one shim: one model per process (see the module docstring).
        memidx.load_embedding_model = self._load_model  # type: ignore[assignment]

    # ---- engine plumbing --------------------------------------------------

    def _load_model(self):
        if self._model is None:
            from fastembed import TextEmbedding

            self._model = TextEmbedding(model_name=memidx.EMBED_MODEL_NAME)
            self._fp = memidx.embedding_fingerprint(self._model)
        return self._model, self._fp

    def _args(self, **kw: Any) -> argparse.Namespace:
        base = {"project": PROJECT, "db": str(self.db), "debug": True}
        base.update(kw)
        return argparse.Namespace(**base)

    def _write(self, topic: Topic) -> None:
        path = topic.path(self.root)
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(topic.render(), encoding="utf-8")
        self.dirty = True

    def _reindex(self) -> None:
        if not self.dirty:
            return
        out, err = io.StringIO(), io.StringIO()
        with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
            rc = memidx.cmd_reindex(
                self._args(root=str(self.root), full=False, no_embed=False, auto=False)
            )
        if rc != 0:
            raise RuntimeError(f"memcontinuum reindex failed ({rc}): {err.getvalue()}")
        if memidx.reindex_embedding_backend_failed():
            raise RuntimeError(f"memcontinuum reindex: embeddings unavailable: {err.getvalue()}")
        for line in err.getvalue().splitlines():
            if "WARNING" in line or "quarantin" in line:
                raise RuntimeError(f"memcontinuum reindex warned: {line}")
        self.dirty = False
        self.reindexes += 1

    def _topic_of(self, key: str) -> Topic:
        try:
            return self.topics[self.key_topic[key]]
        except KeyError as e:
            raise KeyError(f"unknown key {key}") from e

    # ---- lifecycle --------------------------------------------------------

    def reset(self) -> None:
        shutil.rmtree(self.root, ignore_errors=True)
        self.root.mkdir(parents=True, exist_ok=True)
        (self.root / "topics").mkdir()
        if self.db.exists():
            self.db.unlink()
        self.db.parent.mkdir(parents=True, exist_ok=True)
        self.topics.clear()
        self.key_topic.clear()
        self.dirty = True
        self._reindex()  # an initialized, empty index

    def close(self) -> None:
        pass

    def capabilities(self) -> dict[str, bool]:
        return {
            "link": True,  # typed edges on links; a hit carries its edge-neighbours (see recall)
            "history": True,  # the chain: memidx chain / the topic's own link list
            "trace": False,  # a historical ruling leaves the file but default search hides it
            "suspects": False,
            "temporal": False,  # search filters by status/type/area/topic/authority, not by date
            "verdict": False,
            "write_check": False,
            "endorse_retrieval": False,
            "endorse_assistant": False,  # only the owner promotes (SCHEMA §5)
            "endorse_user": True,  # promotion: a new owner-ratified link, promoted_by on the old
            "endorse_supervisor": False,
        }

    # ---- writes -----------------------------------------------------------

    def _new_topic(self, record: dict[str, Any], source: str) -> Topic:
        key = record["key"]
        topic = Topic(
            tid=key,
            title=record["title"],
            area=str(record.get("kind") or "note").lower(),
            code_refs=list(record.get("code_refs") or []),
            tags=[str(record.get("kind") or "note")],
        )
        day = _day(record.get("created_at"))
        topic.links.append(
            {
                "link": "L1",
                "date": day,
                "status": "active",
                "kind": "adopted",
                "ruling": {"text": record["body"], "authority": "agent-inference", "source": source},
                "recorded_by": "agent",
                "recorded_at": day,
            }
        )
        topic.key_of["L1"] = key
        self.topics[key] = topic
        self.key_topic[key] = key
        return topic

    def inscribe(self, record: dict[str, Any], mode: str) -> Inscribed:
        topic = self._new_topic(record, f"knowledgedrift {mode}")
        self._write(topic)
        return Inscribed()

    # The schema's seven edge relations have no "because"; each harness
    # verb is written as the one true reading the vocabulary allows, on the
    # side whose sentence it is:
    #   X because Y    -> Y led_to X       (the reason led to the decision)
    #   R answers P    -> P led_to R       (the problem led to the resolution)
    #   X builds-on Y  -> Y led_to X
    #   X about Y      -> X applies_to Y
    EDGE_OF = {
        "because": ("led_to", True),
        "answers": ("led_to", True),
        "builds-on": ("led_to", True),
        "about": ("applies_to", False),
    }

    def link(self, from_key: str, to_key: str, verb: str) -> bool:
        rel, reverse = self.EDGE_OF.get(verb, (None, False))
        if rel is None:
            return False
        src, dst = (to_key, from_key) if reverse else (from_key, to_key)
        topic, target = self._topic_of(src), self._topic_of(dst)
        link = topic.link_for_key(src)
        if link is None:
            raise KeyError(f"link: {src} has no link in topic {topic.id}")
        link.setdefault("edges", []).append({"rel": rel, "to": target.id})
        self._write(topic)
        return True

    def supersede(self, old_key: str, new_record: dict[str, Any]) -> None:
        topic = self._topic_of(old_key)
        old = topic.link_for_key(old_key)
        if old is None:
            raise KeyError(f"supersede: {old_key} has no link in topic {topic.id}")
        lid = topic.next_link()
        day = _day(new_record.get("created_at"))
        old["status"] = "superseded"
        old["superseded_by"] = lid
        topic.links.append(
            {
                "link": lid,
                "date": day,
                "status": "active",
                "kind": "reversed",
                "reverses": old["link"],
                "reason_for_change": "changed-mind",
                "ruling": {
                    "text": new_record["body"],
                    "authority": "agent-inference",
                    "source": "knowledgedrift supersede",
                },
                "recorded_by": "agent",
                "recorded_at": day,
            }
        )
        topic.key_of[lid] = new_record["key"]
        topic.title = new_record["title"]  # title changes are free (SCHEMA §7)
        self.key_topic[new_record["key"]] = topic.id
        self._write(topic)

    def release(self, key: str, reason: str) -> None:
        topic = self._topic_of(key)
        link = topic.link_for_key(key)
        if link is None:
            raise KeyError(f"release: {key} has no link in topic {topic.id}")
        # The lifecycle move active -> historical ("no longer applicable,
        # nothing replaced it"); the reason goes into the free body text,
        # the only place the schema lets prose land after the fact.
        for other in topic.links:
            if other.get("status") == "active":
                other["status"] = "historical"
        topic.notes.append(f"\nReleased {_day(None)}: {reason}")
        self._write(topic)

    def purge(self, key: str) -> None:
        topic = self._topic_of(key)
        path = topic.path(self.root)
        if path.exists():
            path.unlink()
        for k, tid in list(self.key_topic.items()):
            if tid == topic.id:
                del self.key_topic[k]
        del self.topics[topic.id]
        self.dirty = True

    def endorse(self, key: str, by: str) -> bool:
        if by != "user":
            return False
        topic = self._topic_of(key)
        link = topic.link_for_key(key)
        if link is None or link.get("status") != "active":
            return True  # nothing active to promote
        # SCHEMA §5: the owner affirms the exact text; a new owner-ratified
        # link is appended and the old link gets promoted_by.
        lid = topic.next_link()
        day = _day(None)
        link["promoted_by"] = lid
        topic.links.append(
            {
                "link": lid,
                "date": day,
                "status": "active",
                "kind": "adopted",
                "ruling": {
                    "text": (link.get("ruling") or {}).get("text", ""),
                    "authority": "owner-ratified",
                    "source": f"ratification sitting {day}",
                },
                "recorded_by": "agent",
                "recorded_at": day,
            }
        )
        topic.key_of[lid] = key
        self._write(topic)
        return True

    def settle(self) -> Optional[str]:
        self._reindex()
        return f"reindexed {self.reindexes} times"

    # ---- reads ------------------------------------------------------------

    def recall(self, query: str, k: int, window: Optional[dict[str, int]]) -> Recalled:
        self._reindex()
        out, err = io.StringIO(), io.StringIO()
        args = self._args(
            root=None,
            query=query,
            mode=self.mode,
            status=[],
            type=[],
            area=None,
            topic=None,
            authority=None,
            limit=k,
            json=True,
            include_inbox=False,
        )
        with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
            rc = memidx.cmd_search(args)
        if rc != 0:
            raise RuntimeError(f"memcontinuum search failed ({rc}): {err.getvalue()}")
        payload = json.loads(out.getvalue() or "[]")
        if isinstance(payload, dict):
            if payload.get("embedding"):
                raise RuntimeError(f"memcontinuum search degraded: {payload}")
            payload = payload.get("results", [])
        neighbours = self._edge_neighbours([entry["id"] for entry in payload])
        hits: list[Hit] = []
        for entry in payload:
            topic = self.topics.get(entry["id"])
            key = None
            created = None
            if topic is not None:
                link = None
                if entry.get("matched_link_id"):
                    link = next((l for l in topic.links if l["link"] == entry["matched_link_id"]), None)
                if link is None:
                    link = topic.current()
                if link is not None:
                    key = topic.key_of.get(link["link"])
                    created = _day_unix(link["date"])
            hits.append(
                Hit(
                    key=key,
                    text=f"{entry['title']}\n{entry.get('snippet', '')}",
                    score=float(entry["score"]),
                    created_at=created,
                    neighbors=neighbours.get(entry["id"], []),
                )
            )
        return Recalled(hits=hits)

    def _current_key(self, topic_id: str) -> Optional[str]:
        topic = self.topics.get(topic_id)
        if topic is None:
            return None
        link = topic.current()
        return topic.key_of.get(link["link"]) if link is not None else None

    def _edge_neighbours(self, topic_ids: list[str]) -> dict[str, list[str]]:
        """The typed edges touching each topic, read off the index's own
        ``edges`` table in both directions (``chain --json`` shows a link's
        outgoing edges; the incoming side is the same table queried by
        ``to_ref``). A neighbour is delivered as its topic's current key."""
        if not topic_ids:
            return {}
        conn = sqlite3.connect(self.db)
        try:
            marks = ",".join("?" * len(topic_ids))
            rows = conn.execute(
                f"SELECT from_ref, to_ref FROM edges WHERE project=? AND "
                f"(substr(from_ref, 1, instr(from_ref, '/') - 1) IN ({marks}) OR to_ref IN ({marks}))",
                (PROJECT, *topic_ids, *topic_ids),
            ).fetchall()
        finally:
            conn.close()
        out: dict[str, list[str]] = {}
        for from_ref, to_ref in rows:
            a = from_ref.split("/", 1)[0]
            b = to_ref.split("/", 1)[0].split("#", 1)[0]
            for me, other in ((a, b), (b, a)):
                if me in topic_ids and other != me:
                    key = self._current_key(other)
                    if key is not None and key not in out.setdefault(me, []):
                        out[me].append(key)
        return out

    def recall_path(self, path: str, k: int) -> Recalled:
        """The system's primary channel: ``for-path``, what the pre-edit hook
        injects for a file — every topic whose ``code_refs`` cover the path,
        each delivered as its chain text (the exact lines the hook prints),
        in index order, unranked. The first ``k`` are returned."""
        self._reindex()
        out, err = io.StringIO(), io.StringIO()
        args = self._args(root=str(self.root), file_path=path, json=True, with_chain_text=True)
        with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
            rc = memidx.cmd_for_path(args)
        if rc not in (0, 4):
            raise RuntimeError(f"memcontinuum for-path failed ({rc}): {err.getvalue()}")
        payload = json.loads(out.getvalue() or "{}")
        results = payload.get("results", []) if isinstance(payload, dict) else payload
        chain_text = payload.get("chain_text", "") if isinstance(payload, dict) else ""
        # The hook's text, split back into one block per topic: a topic's
        # block starts with its unindented head line.
        blocks: dict[str, str] = {}
        current: Optional[str] = None
        for line in chain_text.splitlines():
            if line and not line.startswith(" "):
                current = line.split(" ", 1)[0]
                blocks[current] = line
            elif current is not None:
                blocks[current] += "\n" + line
        topic_ids = [r["id"] for r in results if r.get("kind") != "concept"][:k]
        neighbours = self._edge_neighbours(topic_ids)
        hits: list[Hit] = []
        for tid in topic_ids:
            topic = self.topics.get(tid)
            link = topic.current() if topic is not None else None
            hits.append(
                Hit(
                    key=self._current_key(tid),
                    text=blocks.get(tid, tid),
                    score=None,
                    created_at=_day_unix(link["date"]) if link is not None else None,
                    neighbors=neighbours.get(tid, []),
                )
            )
        return Recalled(hits=hits)

    def suspects(self) -> Optional[list[dict[str, Any]]]:
        return None

    def lineage(self, key: str) -> Optional[list[str]]:
        topic = self._topic_of(key)
        head = topic.link_for_key(key)
        if head is None:
            return []
        out: list[str] = []
        for link in reversed(topic.links):
            if link is head:
                continue
            k = topic.key_of.get(link["link"])
            if k and k != key and k not in out:
                out.append(k)
        return out

    def standing_tokens(self) -> int:
        # MemContinuum's hooks inject `for-path` results per edited file,
        # not a standing memory file: nothing is paid before a question.
        return 0
