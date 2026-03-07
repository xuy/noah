#!/usr/bin/env python3
"""
OpenClaw playbook flow regression checker.

Reads Noah's journal.db and validates the latest (or selected) session against
OpenClaw setup governance expectations.

Usage:
  python3 tests/openclaw_flow_check.py
  python3 tests/openclaw_flow_check.py --session-id <id>
  python3 tests/openclaw_flow_check.py --db-path "$HOME/Library/Application Support/com.itman.app/journal.db"
"""

from __future__ import annotations

import argparse
import json
import re
import sqlite3
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any


DEFAULT_DB = Path.home() / "Library/Application Support/com.itman.app/journal.db"


@dataclass
class Finding:
    level: str
    message: str


def parse_args() -> argparse.Namespace:
    p = argparse.ArgumentParser()
    p.add_argument("--db-path", default=str(DEFAULT_DB))
    p.add_argument("--session-id", default=None)
    p.add_argument(
        "--expect-prompt-substring",
        default="openclaw",
        help="Require at least one user message in session containing this (case-insensitive).",
    )
    return p.parse_args()


def latest_session_id(conn: sqlite3.Connection) -> str:
    row = conn.execute("SELECT id FROM sessions ORDER BY created_at DESC LIMIT 1").fetchone()
    if not row:
        raise RuntimeError("No sessions found in database")
    return row[0]


def latest_session_id_matching_user_text(conn: sqlite3.Connection, needle: str) -> str | None:
    row = conn.execute(
        """
        SELECT s.id
        FROM sessions s
        JOIN messages m ON m.session_id = s.id
        WHERE m.role = 'user' AND lower(m.content) LIKE '%' || ? || '%'
        ORDER BY s.created_at DESC
        LIMIT 1
        """,
        (needle.lower(),),
    ).fetchone()
    return row[0] if row else None


def load_traces(conn: sqlite3.Connection, session_id: str) -> list[dict[str, Any]]:
    rows = conn.execute(
        "SELECT timestamp, response FROM llm_traces WHERE session_id = ? ORDER BY timestamp ASC",
        (session_id,),
    ).fetchall()
    traces = []
    for ts, raw in rows:
        try:
            payload = json.loads(raw)
        except json.JSONDecodeError:
            continue
        traces.append({"timestamp": ts, "payload": payload})
    return traces


def load_messages(conn: sqlite3.Connection, session_id: str) -> list[tuple[str, str, str]]:
    return conn.execute(
        "SELECT timestamp, role, content FROM messages WHERE session_id = ? ORDER BY timestamp ASC",
        (session_id,),
    ).fetchall()


def extract_tool_calls(traces: list[dict[str, Any]]) -> list[dict[str, Any]]:
    calls: list[dict[str, Any]] = []
    for t in traces:
        payload = t["payload"]
        for block in payload.get("content", []):
            if block.get("type") == "tool_use":
                calls.append(
                    {
                        "timestamp": t["timestamp"],
                        "name": block.get("name", ""),
                        "input": block.get("input", {}) or {},
                    }
                )
    return calls


def extract_assistant_texts(traces: list[dict[str, Any]]) -> list[tuple[str, str]]:
    out: list[tuple[str, str]] = []
    for t in traces:
        payload = t["payload"]
        for block in payload.get("content", []):
            if block.get("type") == "text":
                out.append((t["timestamp"], block.get("text", "")))
    return out


def main() -> int:
    args = parse_args()
    db_path = Path(args.db_path).expanduser()
    if not db_path.exists():
        print(f"ERROR: db not found: {db_path}")
        return 2

    conn = sqlite3.connect(str(db_path))
    if args.session_id:
        session_id = args.session_id
    else:
        if args.expect_prompt_substring:
            session_id = latest_session_id_matching_user_text(conn, args.expect_prompt_substring)
        else:
            session_id = None
        if not session_id:
            session_id = latest_session_id(conn)
    traces = load_traces(conn, session_id)
    msgs = load_messages(conn, session_id)
    calls = extract_tool_calls(traces)
    texts = extract_assistant_texts(traces)

    findings: list[Finding] = []

    # Scope sanity
    if args.expect_prompt_substring:
        needle = args.expect_prompt_substring.lower()
        if not any(role == "user" and needle in (content or "").lower() for _, role, content in msgs):
            findings.append(Finding("error", f"Session does not appear to be an OpenClaw run (missing user text containing '{needle}')"))

    # 1) Playbook activation required
    pb_active = any(
        c["name"] == "activate_playbook"
        and isinstance(c["input"], dict)
        and c["input"].get("name") == "openclaw-install-config"
        for c in calls
    )
    if not pb_active:
        findings.append(Finding("error", "Missing activate_playbook(openclaw-install-config)"))

    # 2) Structured response required at least once
    structured = any("[SITUATION]" in txt and "[PLAN]" in txt and "[ACTION:" in txt for _, txt in texts)
    if not structured:
        findings.append(Finding("error", "Missing structured [SITUATION]/[PLAN]/[ACTION] response"))

    # 3) Do not run interactive OpenClaw config wizards via shell_run
    interactive_cmd = re.compile(r"\bopenclaw\s+(config|configure)\b(?!\s+--help)", re.IGNORECASE)
    bad_calls = []
    for c in calls:
        if c["name"] != "shell_run":
            continue
        cmd = str(c["input"].get("command", ""))
        if interactive_cmd.search(cmd):
            bad_calls.append((c["timestamp"], cmd))
    if bad_calls:
        findings.append(
            Finding(
                "error",
                "Interactive OpenClaw wizard executed via shell_run: "
                + "; ".join(f"{ts}: `{cmd}`" for ts, cmd in bad_calls[:3]),
            )
        )

    # 4) Done-state must not be wizard-only handoff
    done_texts = [txt for _, txt in texts if "[DONE]" in txt]
    for txt in done_texts:
        low = txt.lower()
        if "wizard" in low or "configure it through" in low or "when it opens" in low:
            findings.append(Finding("error", "Found wizard-handoff [DONE], which violates playbook completion criteria"))
            break

    # Print summary
    print(f"Session: {session_id}")
    print(f"Traces: {len(traces)} | Tool calls: {len(calls)} | Assistant text blocks: {len(texts)}")

    if not findings:
        print("PASS: OpenClaw flow checks passed.")
        return 0

    # Order: errors first
    for f in findings:
        print(f"{f.level.upper()}: {f.message}")
    return 1


if __name__ == "__main__":
    sys.exit(main())
