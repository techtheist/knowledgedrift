"""The adapter protocol, in Python.

An external memory system is scored by KnowledgeDrift by replaying an
exported script (``knowledgedrift --export DIR``) through a subclass of
:class:`MemoryAdapter` and handing the resulting transcript to the Rust
grader (``knowledgedrift --grade transcript.json --script script.json``).
The runner (``run.py``) owns the loop, the timings and the transcript
shape; an adapter owns only the mapping from the twelve operations to its
system's native API.

Keys
----
Every note the script writes carries a ``key`` (``f0012``, ``s-f0012``,
``c3a``, ``z-f0040``). Later operations address notes by key, so an adapter
keeps its own ``key -> native id`` map and never resolves a target by
searching for it. Hits must carry the key of the note they show
(``None`` for anything the system minted itself).

What the grader reads off a hit
-------------------------------
``text``: exactly what the system would put in front of a caller — a
snippet if it shows snippets, the whole note if it shows notes. This is
what the attention columns bill. ``score``: the system's own relevance, if
any. ``created_at``: unix seconds when the system stores capture time (the
temporal family needs it). ``tombstone``: the hit is a deletion marker, not
live knowledge. ``neighbors``: keys of notes the hit carries as 1-hop
context (a graph system's delivery; flat systems leave it empty).

Capabilities
------------
``link``, ``history``, ``trace``, ``suspects``, ``temporal``, ``verdict``,
``write_check``, and the four endorsement rungs ``endorse_retrieval``,
``endorse_assistant``, ``endorse_user``, ``endorse_supervisor`` — declare
what the system can do; the grader marks the families (and, for the
authority family, the tasks) that need a missing one N/A (and the headline
charges for them, so declare honestly, not generously). A rung is declared
when an endorsement on it is stored as a first-class attribute of the note
(a use counter, a confirmed stamp, an approval, a pin); whether ranking
reads it is what the family measures.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any, Optional


@dataclass
class Hit:
    key: Optional[str]
    text: str
    score: Optional[float] = None
    created_at: Optional[int] = None
    tombstone: bool = False
    neighbors: list[str] = field(default_factory=list)

    def to_json(self) -> dict[str, Any]:
        d: dict[str, Any] = {"text": self.text}
        if self.key is not None:
            d["key"] = self.key
        if self.score is not None:
            d["score"] = float(self.score)
        if self.created_at is not None:
            d["created_at"] = int(self.created_at)
        if self.tombstone:
            d["tombstone"] = True
        if self.neighbors:
            d["neighbors"] = list(self.neighbors)
        return d


@dataclass
class Recalled:
    hits: list[Hit]
    declined: bool = False
    dump: bool = False

    def to_json(self) -> dict[str, Any]:
        d: dict[str, Any] = {"hits": [h.to_json() for h in self.hits]}
        if self.declined:
            d["declined"] = True
        if self.dump:
            d["dump"] = True
        return d


@dataclass
class Inscribed:
    """A write verdict. Flat systems return the default (created silently)."""

    matched: Optional[str] = None
    nli_label: Optional[str] = None
    warnings: list[dict[str, Any]] = field(default_factory=list)  # {"reason", "key"?}
    suspects: list[dict[str, Any]] = field(default_factory=list)  # {"a", "b", "hint"?}

    def to_json(self) -> dict[str, Any]:
        d: dict[str, Any] = {}
        if self.matched is not None:
            d["matched"] = self.matched
        if self.nli_label is not None:
            d["nli_label"] = self.nli_label
        if self.warnings:
            d["warnings"] = self.warnings
        if self.suspects:
            d["suspects"] = self.suspects
        return d


CAPABILITY_NAMES = (
    "link",
    "history",
    "trace",
    "suspects",
    "temporal",
    "verdict",
    "write_check",
    "endorse_retrieval",
    "endorse_assistant",
    "endorse_user",
    "endorse_supervisor",
)

AUTHORITIES = ("retrieval", "assistant", "user", "supervisor")


class MemoryAdapter:
    """Subclass this. Every method that a flat store cannot honour has a
    default that says so; override what the system can actually do."""

    name = "adapter"

    def capabilities(self) -> dict[str, bool]:
        return {n: False for n in CAPABILITY_NAMES}

    # ---- writes -----------------------------------------------------------

    def inscribe(self, record: dict[str, Any], mode: str) -> Inscribed:
        """``record``: key, kind, title, body, code_refs, created_at, open.
        ``mode``: "import" (bulk load) or "write" (assistant-style note with
        the system's write-time checks, if any)."""
        raise NotImplementedError

    def link(self, from_key: str, to_key: str, verb: str) -> bool:
        return False

    def supersede(self, old_key: str, new_record: dict[str, Any]) -> None:
        """``new`` re-decides ``old``. A system with history keeps ``old``
        reachable from ``new``; a flat system replaces it in place."""
        raise NotImplementedError

    def release(self, key: str, reason: str) -> None:
        """Retire deliberately, leaving whatever trace the system leaves.
        A system without traces deletes."""
        raise NotImplementedError

    def purge(self, key: str) -> None:
        raise NotImplementedError

    def endorse(self, key: str, by: str) -> bool:
        """Someone vouches for the note: ``by`` is one of ``retrieval`` (it
        was delivered and used), ``assistant`` (confirmed still true),
        ``user`` (the owner approved it), ``supervisor`` (pinned above every
        other signal). May repeat. Return False when the system has no such
        rung (and leave the matching capability False)."""
        return False

    def settle(self) -> Optional[str]:
        """A session boundary: maintenance, calibration, consolidation.
        Returns a note for the receipt."""
        return None

    # ---- reads ------------------------------------------------------------

    def recall(self, query: str, k: int, window: Optional[dict[str, int]]) -> Recalled:
        """``window``: {"after": unix, "before": unix} half-open, or None."""
        raise NotImplementedError

    def recall_path(self, path: str, k: int) -> Recalled:
        """The path-shaped read (v2 worlds): what a caller about to touch
        ``path`` should see. A system with a file channel (an edit hook, a
        code-ref index, a metadata filter) answers from it; the default
        hands the path to ``recall`` as a query, which is what a store
        without one would do."""
        return self.recall(path, k, None)

    def suspects(self) -> Optional[list[dict[str, Any]]]:
        """Every disagreement the system wants a person to judge, as
        {"a": key, "b": key, "hint": "contradiction"|...}. None = no such
        concept."""
        return None

    def lineage(self, key: str) -> Optional[list[str]]:
        """Keys reachable from ``key`` through its supersession history.
        None = the system keeps no history."""
        return None

    def standing_tokens(self) -> int:
        """Tokens the system costs every session before a question is asked."""
        return 0

    # ---- lifecycle --------------------------------------------------------

    def reset(self) -> None:
        """Start from an empty store. Called once before the script runs."""
        raise NotImplementedError

    def close(self) -> None:
        pass


def tokens(s: str) -> int:
    """The grader's crude estimate (chars / 4), for adapters that want to
    report a standing cost in the same units."""
    return -(-len(s) // 4)
