#!/usr/bin/env python3
"""
claude-session-bridge.py — Claude Code session → CORPUS bridge.

Watches ~/.claude/projects/**/*.jsonl for new assistant turns and writes
CORPUS_claude_<session_id>_<turn_id>.json into the service-content watched
directory so OLMo can extract entities from Claude's text.

ToS compliance: OLMo extracts entities FROM Claude's text as a source document
(the same as ingesting any CORPUS file — emails, project notes). Claude outputs
are not used as training signal for OLMo. This is extraction, not fine-tuning.

Run as: python3 claude-session-bridge.py
Or as systemd unit: local-claude-bridge.service

Environment:
  CLAUDE_PROJECTS_DIR   default: ~/.claude/projects
  CORPUS_WATCH_DIR      default: /srv/foundry/clones/project-intelligence/data/corpus-watch
  POLL_INTERVAL_SECS    default: 10
"""

import json
import os
import sys
import time
import glob
import hashlib
from pathlib import Path
from datetime import datetime, timezone

CLAUDE_PROJECTS_DIR = Path(os.environ.get(
    "CLAUDE_PROJECTS_DIR",
    Path.home() / ".claude" / "projects"
))
CORPUS_WATCH_DIR = Path(os.environ.get(
    "CORPUS_WATCH_DIR",
    "/srv/foundry/clones/project-intelligence/data/corpus-watch"
))
POLL_INTERVAL_SECS = int(os.environ.get("POLL_INTERVAL_SECS", "10"))

# Track the last byte position for each session file so we only process new lines.
_file_offsets: dict[str, int] = {}

def session_id_from_path(path: Path) -> str:
    # Use the parent directory name (hash of project path) + stem of the file.
    return f"{path.parent.name}_{path.stem}"

def turn_id(content: str, index: int) -> str:
    digest = hashlib.sha256(content.encode()).hexdigest()[:12]
    return f"{index:06d}_{digest}"

def write_corpus(session_id: str, t_id: str, text: str) -> None:
    CORPUS_WATCH_DIR.mkdir(parents=True, exist_ok=True)
    filename = f"CORPUS_claude_{session_id}_{t_id}.json"
    dest = CORPUS_WATCH_DIR / filename
    payload = {
        "source": "claude-session-bridge",
        "session_id": session_id,
        "turn_id": t_id,
        "ingested_at": datetime.now(timezone.utc).isoformat(),
        "text": text,
    }
    # Atomic write via temp file.
    tmp = dest.with_suffix(".tmp")
    tmp.write_text(json.dumps(payload, ensure_ascii=False), encoding="utf-8")
    tmp.rename(dest)
    print(f"[bridge] wrote {filename} ({len(text)} chars)", flush=True)

def extract_assistant_text(entry: dict) -> str | None:
    """Extract plain text from a Claude Code session JSONL entry."""
    # Claude Code JSONL format: {"type": "message", "message": {...}, ...}
    msg = entry.get("message") or entry
    if msg.get("role") != "assistant":
        return None
    content = msg.get("content", "")
    if isinstance(content, str):
        return content.strip() or None
    if isinstance(content, list):
        parts = []
        for block in content:
            if isinstance(block, dict) and block.get("type") == "text":
                parts.append(block.get("text", ""))
        text = "\n".join(parts).strip()
        return text or None
    return None

def poll_file(path: Path) -> None:
    session_id = session_id_from_path(path)
    offset = _file_offsets.get(str(path), 0)
    try:
        stat = path.stat()
    except FileNotFoundError:
        return

    if stat.st_size <= offset:
        return

    try:
        with path.open("r", encoding="utf-8", errors="replace") as f:
            f.seek(offset)
            index = _file_offsets.get(f"{path}.index", 0)
            for line in f:
                try:
                    entry = json.loads(line)
                    text = extract_assistant_text(entry)
                    if text and len(text) >= 50:  # skip trivial one-liners
                        t_id = turn_id(text, index)
                        write_corpus(session_id, t_id, text)
                    index += 1
                except json.JSONDecodeError:
                    pass
            new_offset = f.tell()
        _file_offsets[str(path)] = new_offset
        _file_offsets[f"{path}.index"] = index
    except (OSError, PermissionError) as e:
        print(f"[bridge] warn: could not read {path}: {e}", file=sys.stderr, flush=True)

def discover_session_files() -> list[Path]:
    pattern = str(CLAUDE_PROJECTS_DIR / "**" / "*.jsonl")
    return [Path(p) for p in glob.glob(pattern, recursive=True)]

def main() -> None:
    print(f"[bridge] watching {CLAUDE_PROJECTS_DIR} → {CORPUS_WATCH_DIR}", flush=True)
    print(f"[bridge] poll interval: {POLL_INTERVAL_SECS}s", flush=True)
    while True:
        for path in discover_session_files():
            poll_file(path)
        time.sleep(POLL_INTERVAL_SECS)

if __name__ == "__main__":
    main()
