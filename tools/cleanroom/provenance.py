"""Shared context and append helpers for the clean-room provenance log.

Used by log.py (PostToolUse) and guard.py (PreToolUse). Python 3 stdlib
only (SPEC S2.3).
"""

from __future__ import annotations

import fcntl
import json
import os
import re
import subprocess
import sys
from datetime import datetime, timezone

ENTRY_KEYS = (
    "ts",
    "foundry_stage",
    "task_id",
    "model",
    "tool",
    "target",
    "decision",
    "reason",
)

_VALID_STAGES = {"plan", "implement", "review", "fix", "summarize"}

_TASK_ID_RE = re.compile(r"^- \[~\] (\S+)")


def now_iso() -> str:
    """UTC ISO-8601 timestamp with millisecond precision and a Z suffix."""
    dt = datetime.now(timezone.utc)
    return dt.strftime("%Y-%m-%dT%H:%M:%S.") + f"{dt.microsecond // 1000:03d}Z"


def repo_root(hook_input: dict) -> str:
    """Resolve the repository root.

    Order: CLEANROOM_REPO_ROOT env, CLAUDE_PROJECT_DIR env, the git
    toplevel of hook_input["cwd"], else the current working directory.
    """
    env_root = os.environ.get("CLEANROOM_REPO_ROOT")
    if env_root:
        return env_root
    project_dir = os.environ.get("CLAUDE_PROJECT_DIR")
    if project_dir:
        return project_dir
    cwd = hook_input.get("cwd") if isinstance(hook_input, dict) else None
    if cwd:
        try:
            out = subprocess.run(
                ["git", "-C", cwd, "rev-parse", "--show-toplevel"],
                capture_output=True,
                text=True,
                timeout=5,
                check=False,
            )
            if out.returncode == 0:
                top = out.stdout.strip()
                if top:
                    return top
        except Exception:
            pass
    return os.getcwd()


def log_path(root: str) -> str:
    """Resolve the provenance log path: CLEANROOM_LOG env, else
    <root>/provenance/tool-log.jsonl."""
    env_log = os.environ.get("CLEANROOM_LOG")
    if env_log:
        return env_log
    return os.path.join(root, "provenance", "tool-log.jsonl")


def _round_from_state(root: str) -> int | None:
    state_path = os.path.join(root, ".foundry", "state.json")
    try:
        with open(state_path, "r", encoding="utf-8") as f:
            data = json.load(f)
        round_val = data.get("round")
        if isinstance(round_val, int):
            return round_val
    except Exception:
        pass
    return None


def _stage_from_agent_type(agent_type: str, root: str) -> str | None:
    at = agent_type.lower()
    if "planner" in at:
        return "plan"
    if "implementer" in at:
        round_val = _round_from_state(root)
        if round_val is not None and round_val >= 1:
            return "fix"
        return "implement"
    if "reviewer" in at:
        return "review"
    if "summarizer" in at:
        return "summarize"
    return None


def derive_stage(hook_input: dict, root: str) -> str | None:
    """First-match-wins stage derivation. See P0-03 for the rules."""
    agent_type = None
    if isinstance(hook_input, dict):
        agent_type = hook_input.get("agent_type")
    if isinstance(agent_type, str) and agent_type:
        stage = _stage_from_agent_type(agent_type, root)
        if stage is not None:
            return stage

    env_stage = os.environ.get("FOUNDRY_STAGE")
    if env_stage in _VALID_STAGES:
        return env_stage

    lock_path = os.path.join(root, ".foundry", "implement.lock")
    if os.path.exists(lock_path):
        round_val = _round_from_state(root)
        if round_val is not None and round_val >= 1:
            return "fix"
        return "implement"

    return None


def derive_task_id(root: str, stage: str | None) -> str | None:
    """Task derivation: FOUNDRY_TASK_ID env when stage is implement/fix,
    else the first in-progress task line in docs/PROGRESS.md."""
    if stage in ("implement", "fix"):
        env_task = os.environ.get("FOUNDRY_TASK_ID")
        if env_task:
            return env_task
    progress_path = os.path.join(root, "docs", "PROGRESS.md")
    try:
        with open(progress_path, "r", encoding="utf-8") as f:
            in_tasks = False
            for line in f:
                if line.strip() == "## Tasks":
                    in_tasks = True
                    continue
                if in_tasks:
                    if line.startswith("## ") and line.strip() != "## Tasks":
                        break
                    m = _TASK_ID_RE.match(line)
                    if m:
                        return m.group(1)
    except Exception:
        pass
    return None


def _model_from_transcript(transcript_path: str) -> str | None:
    try:
        size = os.path.getsize(transcript_path)
        with open(transcript_path, "rb") as f:
            if size > 256 * 1024:
                f.seek(size - 256 * 1024)
            data = f.read()
        text = data.decode("utf-8", errors="ignore")
        lines = text.splitlines()
        for line in reversed(lines):
            line = line.strip()
            if not line:
                continue
            try:
                obj = json.loads(line)
            except Exception:
                continue
            message = obj.get("message") if isinstance(obj, dict) else None
            if isinstance(message, dict):
                model = message.get("model")
                if isinstance(model, str) and model:
                    return model
    except Exception:
        return None
    return None


def derive_model(hook_input: dict) -> str | None:
    """model from hook input, else CLEANROOM_MODEL env, else best-effort
    transcript tail scan."""
    if isinstance(hook_input, dict):
        model = hook_input.get("model")
        if isinstance(model, str) and model:
            return model
    env_model = os.environ.get("CLEANROOM_MODEL")
    if env_model:
        return env_model
    transcript_path = (
        hook_input.get("transcript_path") if isinstance(hook_input, dict) else None
    )
    if isinstance(transcript_path, str) and transcript_path:
        return _model_from_transcript(transcript_path)
    return None


def context(hook_input: dict, root: str) -> dict:
    """Build the foundry_stage/task_id/model portion of a log entry."""
    stage = derive_stage(hook_input, root)
    task_id = derive_task_id(root, stage)
    model = derive_model(hook_input)
    return {
        "foundry_stage": stage,
        "task_id": task_id,
        "model": model,
    }


def _truncate(s: str, n: int) -> str:
    if len(s) > n:
        return s[:n]
    return s


def target_of(tool_name: str, tool_input: dict) -> str:
    """Map a tool name and input to the logged target string."""
    if not isinstance(tool_input, dict):
        tool_input = {}
    if tool_name in ("Read", "Write", "Edit"):
        return str(tool_input.get("file_path", ""))
    if tool_name == "NotebookEdit":
        return str(tool_input.get("notebook_path", ""))
    if tool_name == "Grep":
        p = tool_input.get("path", "")
        g = tool_input.get("glob", "")
        pat = tool_input.get("pattern", "")
        return f"path={p} glob={g} pattern={pat}"
    if tool_name == "Glob":
        p = tool_input.get("path", "")
        pat = tool_input.get("pattern", "")
        return f"path={p} pattern={pat}"
    if tool_name == "WebFetch":
        return str(tool_input.get("url", ""))
    if tool_name == "WebSearch":
        return str(tool_input.get("query", ""))
    if tool_name == "Bash":
        return _truncate(str(tool_input.get("command", "")), 4000)
    if tool_name in ("Task", "Agent"):
        sub = tool_input.get("subagent_type", "")
        desc = tool_input.get("description", "")
        return f"{sub}: {desc}"
    try:
        compact = json.dumps(tool_input, separators=(",", ":"))
    except Exception:
        compact = str(tool_input)
    return _truncate(f"{tool_name} {compact}", 500)


def append(entry: dict, path: str) -> None:
    """Append one JSON line to the log at path, creating provenance/ if
    missing, using O_APPEND|O_CREAT and an flock for atomicity."""
    directory = os.path.dirname(path)
    if directory:
        os.makedirs(directory, exist_ok=True)
    ordered = {k: entry.get(k) for k in ENTRY_KEYS}
    for k, v in entry.items():
        if k not in ENTRY_KEYS:
            ordered[k] = v
    line = json.dumps(ordered, separators=(",", ":")) + "\n"
    fd = os.open(path, os.O_APPEND | os.O_CREAT | os.O_WRONLY, 0o644)
    try:
        fcntl.flock(fd, fcntl.LOCK_EX)
        try:
            os.write(fd, line.encode("utf-8"))
        finally:
            fcntl.flock(fd, fcntl.LOCK_UN)
    finally:
        os.close(fd)


def read_hook_input(stream=None) -> dict:
    """Read and parse the hook JSON payload from stdin (or the given
    stream). Returns {} on any parse failure."""
    if stream is None:
        stream = sys.stdin
    try:
        raw = stream.read()
        data = json.loads(raw)
        if isinstance(data, dict):
            return data
    except Exception:
        pass
    return {}
