#!/usr/bin/env python3
"""Trace retrieval + source numbering for a query against live Ashkorix data."""

import re
import sqlite3
import sys
from pathlib import Path

DATA_DIR = Path(__file__).resolve().parents[2] / "Data"
DB = DATA_DIR / "ashkorix.db"
DOC_ID = "a7b14ec0ade8"
QUERY = sys.argv[1] if len(sys.argv) > 1 else "What is Phase 1?"


def load_chunks(conn, doc_id: str):
    rows = conn.execute(
        """
        SELECT id, chunk_index, section_title, heading_path, text, token_count
        FROM chunks WHERE document_id=? ORDER BY chunk_index
        """,
        (doc_id,),
    ).fetchall()
    return [
        {
            "id": r[0],
            "chunk_index": r[1],
            "section_title": r[2] or "",
            "heading_path": r[3] or "",
            "text": r[4] or "",
            "token_count": r[5] or 0,
        }
        for r in rows
    ]


def lexical_hits(chunks, query: str):
    """Rough lexical rank: count query term hits in text + heading."""
    q = query.lower()
    terms = [t for t in re.split(r"\W+", q) if len(t) > 1]
    scored = []
    for c in chunks:
        text = c["text"].lower()
        heading = c["heading_path"].lower()
        overlap = 0.0
        for term in terms:
            if term in text:
                overlap += 1.0
            if term in heading:
                overlap += 0.5
        if q in text:
            overlap += 2.0
        if overlap > 0:
            scored.append((overlap, c))
    scored.sort(key=lambda x: (-x[0], x[1]["chunk_index"]))
    return scored


def heuristic_rerank(chunks, query: str, rrf_score: float = 0.5):
    q = query.lower()
    terms = [t for t in re.split(r"\W+", q) if len(t) >= 2]
    ranked = []
    for c in chunks:
        text = c["text"].lower()
        heading = c["heading_path"].lower()
        overlap = 0.0
        for term in terms:
            if term in text:
                overlap += 1.0
            if term in heading:
                overlap += 0.5
        if q in text:
            overlap += 2.0
        # Mirror HeuristicReranker phase-heading boost
        m = re.search(r"(?i)\bphase\s+\d+\b", query)
        if m:
            phrase = m.group(0).lower()
            heading = c["heading_path"].lower()
            section = c["section_title"].lower()
            text_l = c["text"].lower()
            if phrase in heading or phrase in section or phrase in text_l:
                overlap += 5.0
        rerank = rrf_score * 0.4 + overlap * 0.6
        ranked.append((rerank, overlap, c))
    ranked.sort(key=lambda x: (-x[0], x[2]["chunk_index"]))
    return ranked


def stable_order(chunks):
    """Mirror ashkorix-core stable_order (document position)."""
    return sorted(
        chunks,
        key=lambda c: (c["chunk_index"],),
    )


def assign_in_order(chunks):
    return list(chunks)


def print_ranking(label, ordered, show_text=100):
    print(f"\n=== {label} ===")
    for i, item in enumerate(ordered, start=1):
        if isinstance(item, tuple):
            score, c = item[0], item[-1]
            extra = f" rerank={score:.3f}" if len(item) >= 2 else ""
        else:
            c, extra = item, ""
        title = c["section_title"] or c["heading_path"][:60]
        preview = c["text"][:show_text].replace("\n", " ")
        print(f"  [Source {i}] chunk_index={c['chunk_index']} id={c['id'][-12:]}{extra}")
        print(f"           {title}")
        print(f"           {preview}...")


def main():
    if not DB.exists():
        print(f"Database not found: {DB}", file=sys.stderr)
        sys.exit(1)

    conn = sqlite3.connect(DB)
    all_chunks = load_chunks(conn, DOC_ID)
    print(f"Query: {QUERY!r}")
    print(f"Document {DOC_ID}: {len(all_chunks)} chunks")

    hits = lexical_hits(all_chunks, QUERY)
    print(f"\nLexical hits in Salutori doc: {len(hits)}")
    for score, c in hits[:6]:
        print(f"  score={score:.1f} idx={c['chunk_index']} {c['section_title'][:50]}")

    # Simulate top retrieved set (both section intro + Phase 1 body likely hit)
    candidate_ids = {c["id"] for _, c in hits[:8]}
    candidates = [c for c in all_chunks if c["id"] in candidate_ids]

    reranked = heuristic_rerank(candidates, QUERY)
    print_ranking("After heuristic rerank (relevance order)", reranked)

    after_stable = stable_order([c for _, _, c in reranked])
    print_ranking("After stable_order (CURRENT — document position)", after_stable)

    after_fix = assign_in_order([c for _, _, c in reranked])
    print_ranking("After assign_source_numbers (FIX — keep rerank order)", after_fix)

    # Verify claim against Source 1 in each case
    claim = (
        "Phase 1 focuses on vectormath and embeddings via ort targeting bge-small-en-v1.5"
    )
    terms = [t for t in re.split(r"\W+", claim.lower()) if len(t) > 4]

    def overlap_ratio(source_text: str) -> float:
        src = source_text.lower()
        matched = sum(1 for t in terms if t in src)
        return matched / len(terms) if terms else 1.0

    for label, ordered in [
        ("stable_order Source 1", after_stable[:1]),
        ("fix Source 1", after_fix[:1]),
    ]:
        c = ordered[0]
        ratio = overlap_ratio(c["text"])
        ok = ratio >= 0.35
        print(
            f"\nClaim verification vs {label} ({c['section_title'][:40]}): "
            f"overlap={ratio:.0%} -> {'PASS' if ok else 'FAIL'}"
        )


if __name__ == "__main__":
    main()
