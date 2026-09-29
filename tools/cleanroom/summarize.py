#!/usr/bin/env python3
"""Generate `provenance/SUMMARY.md` from the tool log, the
human-interventions file and `provenance/README.md`'s "Known gaps"
section (SPEC S5 M6, S2.3).

Python 3 stdlib only. Usage: `summarize.py [--log PATH] [--out PATH]`.
Without `--log`: `provenance/tool-log.jsonl` if it exists (the live log),
else `provenance/tool-log.snapshot.jsonl` (the committed snapshot).
Without `--out`: `provenance/SUMMARY.md`.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import sys
from datetime import datetime, timezone
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent.parent

# Matches an http(s) URL embedded anywhere in a Bash command line (e.g.
# `curl -sSfL -o x https://host/path`, a fetch-script invocation), stopping
# at whitespace or the shell/quoting characters that commonly terminate a
# URL in a command line.
URL_RE = re.compile(r"https?://[^\s'\"()<>]+")

# Gaps under this many minutes count as "active" time between calls; a
# larger gap is treated as idle (the operator stepped away, a long-running
# command ran, etc.) and excluded from the active-time sum.
ACTIVE_GAP_MINUTES = 15

# Fetch scripts under tools/cleanroom/ and the tracked manifest each one
# writes (SPEC S5 M6: "every file and URL read"). Manifest paths are
# relative to the repo root passed to build_summary, so tests can point
# them at fixture manifests.
FETCH_SCRIPT_MANIFESTS = {
    "fetch-rfcs.sh": "docs/rfc/SOURCES.json",
    "fetch-corpora.sh": "testdata/corpora/SOURCES.json",
}

# Bash command words that only *mention* a fetch script (search it, print
# it, lint it, mark it executable, stage it) rather than run it. A line
# whose first word is one of these never counts as an invocation, even if
# a fetch-script filename appears later in the same command.
MENTION_ONLY_VERBS = {
    "grep",
    "egrep",
    "fgrep",
    "sed",
    "cat",
    "head",
    "tail",
    "less",
    "more",
    "shellcheck",
    "chmod",
    "git",
    "find",
    "ls",
    "wc",
    "diff",
    "stat",
    "echo",
    "printf",
    "file",
    "awk",
}


def _invoked_fetch_scripts(command: str) -> set[str]:
    """Which fetch-script basenames (keys of FETCH_SCRIPT_MANIFESTS) a Bash
    command *invokes* -- e.g. `bash tools/cleanroom/fetch-rfcs.sh`,
    `./tools/cleanroom/fetch-corpora.sh all --record` -- as opposed to
    merely mentioning the filename (grep/sed/cat/shellcheck/chmod/git add
    on it). The command is split on shell separators (`;`, `&&`, `||`,
    `|`, newlines) and each piece is judged independently."""
    invoked: set[str] = set()
    pieces = re.split(r"&&|\|\||[;|\n]", command)
    for piece in pieces:
        words = piece.split()
        if not words:
            continue
        first = words[0]
        first_base = first.rsplit("/", 1)[-1]
        for script_name in FETCH_SCRIPT_MANIFESTS:
            if script_name not in piece:
                continue
            if first_base in ("bash", "sh") and len(words) > 1:
                second_base = words[1].rsplit("/", 1)[-1]
                if second_base == script_name:
                    invoked.add(script_name)
                    continue
            if first_base == script_name:
                invoked.add(script_name)
                continue
            if first_base in MENTION_ONLY_VERBS:
                continue
    return invoked


def _urls_from_rfc_manifest(path: Path) -> list[str]:
    data = json.loads(path.read_text(encoding="utf-8"))
    urls: list[str] = []
    if isinstance(data, list):
        for entry in data:
            if isinstance(entry, dict) and entry.get("url"):
                urls.append(str(entry["url"]))
    return urls


def _urls_from_corpus_manifest(path: Path) -> list[tuple[str, str | None]]:
    """Returns (url, fetched_by) pairs, one per corpus in the manifest."""
    data = json.loads(path.read_text(encoding="utf-8"))
    pairs: list[tuple[str, str | None]] = []
    if isinstance(data, dict):
        for _corpus, info in data.items():
            if not isinstance(info, dict):
                continue
            url = info.get("url")
            if not url:
                continue
            fetched_by = info.get("fetched_by")
            pairs.append((str(url), str(fetched_by) if fetched_by else None))
    return pairs


def render_fetch_scripts(entries: list[dict], root: Path) -> str:
    """Fetch scripts *invoked* by an allowed Bash call, and every URL
    recorded in the tracked manifest each one writes. A denied invocation,
    or a line that merely mentions the script, contributes nothing here."""
    invoked: set[str] = set()
    for e in entries:
        if e.get("decision") != "allowed":
            continue
        if e.get("tool") != "Bash":
            continue
        target = str(e.get("target", ""))
        if not target:
            continue
        invoked |= _invoked_fetch_scripts(target)

    if not invoked:
        return "None recorded."

    lines = []
    for script_name in sorted(invoked):
        manifest_rel = FETCH_SCRIPT_MANIFESTS[script_name]
        manifest_path = root / manifest_rel
        lines.append(f"### `tools/cleanroom/{script_name}` (manifest: `{manifest_rel}`)")
        lines.append("")
        if not manifest_path.is_file():
            lines.append(f"MISSING: `{manifest_rel}` not found.")
            lines.append("")
            continue
        if script_name == "fetch-rfcs.sh":
            for u in _urls_from_rfc_manifest(manifest_path):
                lines.append(f"- {u}")
        else:
            for u, fetched_by in _urls_from_corpus_manifest(manifest_path):
                if fetched_by:
                    lines.append(f"- {u} (fetched_by: {fetched_by})")
                else:
                    lines.append(f"- {u}")
        lines.append("")
    return "\n".join(lines).rstrip()


def default_log_path(root: Path) -> Path:
    live = root / "provenance" / "tool-log.jsonl"
    if live.is_file():
        return live
    return root / "provenance" / "tool-log.snapshot.jsonl"


def parse_ts(ts: str) -> datetime | None:
    if not isinstance(ts, str) or not ts:
        return None
    text = ts
    if text.endswith("Z"):
        text = text[:-1] + "+00:00"
    try:
        dt = datetime.fromisoformat(text)
    except ValueError:
        return None
    if dt.tzinfo is None:
        dt = dt.replace(tzinfo=timezone.utc)
    return dt


def load_entries(log_path: Path) -> tuple[list[dict], int]:
    """Return (parsed entries, count of lines that failed to parse).
    Lines that fail to parse are counted, never dropped silently."""
    entries: list[dict] = []
    bad = 0
    if not log_path.is_file():
        return entries, bad
    with open(log_path, "r", encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            try:
                obj = json.loads(line)
            except Exception:
                bad += 1
                continue
            if not isinstance(obj, dict):
                bad += 1
                continue
            entries.append(obj)
    return entries, bad


def totals_by(entries: list[dict], key: str) -> dict[str, int]:
    counts: dict[str, int] = {}
    for e in entries:
        k = e.get(key)
        k = str(k) if k is not None else "unknown"
        counts[k] = counts.get(k, 0) + 1
    return counts


def render_counts_table(counts: dict[str, int], header: str) -> str:
    lines = [f"| {header} | count |", "|---|---:|"]
    for k in sorted(counts):
        lines.append(f"| {k} | {counts[k]} |")
    lines.append(f"| **total** | **{sum(counts.values())}** |")
    return "\n".join(lines)


def render_denied_table(entries: list[dict]) -> str:
    denied = [e for e in entries if e.get("decision") == "denied"]
    if not denied:
        return "None recorded."
    lines = [
        "| ts | stage | task | tool | target | reason |",
        "|---|---|---|---|---|---|",
    ]
    for e in denied:
        ts = e.get("ts", "")
        stage = e.get("foundry_stage") or "unknown"
        task = e.get("task_id") or "unknown"
        tool = e.get("tool", "")
        target = str(e.get("target", "")).replace("|", "\\|").replace("\n", " ")
        if len(target) > 200:
            target = target[:200] + "..."
        reason = str(e.get("reason", "")).replace("|", "\\|")
        lines.append(f"| {ts} | {stage} | {task} | {tool} | {target} | {reason} |")
    return "\n".join(lines)


def _repo_top_level_dir(path_str: str, root: Path) -> str | None:
    """The first path component of `path_str` relative to `root`, or
    `None` if `path_str` does not resolve under `root`."""
    try:
        p = Path(path_str)
        if not p.is_absolute():
            return None
        resolved = os.path.normpath(str(p))
        root_str = os.path.normpath(str(root))
        if resolved == root_str:
            return "."
        if not resolved.startswith(root_str + os.sep):
            return None
        rel = resolved[len(root_str) + 1 :]
        parts = rel.split(os.sep)
        return parts[0] if parts else "."
    except Exception:
        return None


def render_files_read(entries: list[dict], root: Path) -> str:
    file_tools = ("Read", "Write", "Edit", "NotebookEdit")
    inside: dict[str, set[str]] = {}
    outside: set[str] = set()
    for e in entries:
        if e.get("decision") != "allowed":
            continue
        if e.get("tool") not in file_tools:
            continue
        target = str(e.get("target", ""))
        if not target:
            continue
        top = _repo_top_level_dir(target, root)
        if top is None:
            outside.add(target)
        else:
            inside.setdefault(top, set()).add(target)

    lines = []
    if not inside:
        lines.append("None recorded.")
    else:
        for top in sorted(inside):
            lines.append(f"- `{top}/` ({len(inside[top])} file(s))")
            for path in sorted(inside[top]):
                lines.append(f"  - {path}")
    lines.append("")
    lines.append("### Outside repository")
    lines.append("")
    if not outside:
        lines.append("None recorded.")
    else:
        for path in sorted(outside):
            lines.append(f"- {path}")
    return "\n".join(lines)


def render_urls(entries: list[dict]) -> str:
    """Every http(s) URL in an *allowed* WebFetch or Bash call, grouped by
    tool (SPEC S5 M6: the summary must not claim no URL was fetched when
    the log shows a curl/wget fetch or a fetch-script invocation via
    Bash). Denied calls never contribute a URL here -- they already show
    up in the Denied calls table."""
    by_tool: dict[str, set[str]] = {}
    for e in entries:
        if e.get("decision") != "allowed":
            continue
        tool = e.get("tool")
        target = str(e.get("target", ""))
        if not target:
            continue
        if tool == "WebFetch":
            by_tool.setdefault("WebFetch", set()).add(target)
        elif tool == "Bash":
            found = URL_RE.findall(target)
            if found:
                by_tool.setdefault("Bash", set()).update(found)

    if not by_tool:
        return "None recorded."
    lines = []
    for tool in sorted(by_tool):
        lines.append(f"### {tool}")
        lines.append("")
        for u in sorted(by_tool[tool]):
            lines.append(f"- {u}")
        lines.append("")
    return "\n".join(lines).rstrip()


def stage_wall_clock(entries: list[dict]) -> dict[str, dict]:
    by_stage: dict[str, list[datetime]] = {}
    for e in entries:
        stage = e.get("foundry_stage") or "unknown"
        dt = parse_ts(e.get("ts", ""))
        if dt is None:
            continue
        by_stage.setdefault(stage, []).append(dt)

    result: dict[str, dict] = {}
    for stage, timestamps in by_stage.items():
        timestamps.sort()
        first = timestamps[0]
        last = timestamps[-1]
        span = last - first
        active = 0.0
        for prev, cur in zip(timestamps, timestamps[1:]):
            gap = (cur - prev).total_seconds()
            if 0 <= gap < ACTIVE_GAP_MINUTES * 60:
                active += gap
        result[stage] = {
            "first": first,
            "last": last,
            "span_seconds": span.total_seconds(),
            "active_seconds": active,
        }
    return result


def fmt_duration(seconds: float) -> str:
    seconds = int(seconds)
    hours, rem = divmod(seconds, 3600)
    minutes, secs = divmod(rem, 60)
    if hours:
        return f"{hours}h{minutes}m{secs}s"
    if minutes:
        return f"{minutes}m{secs}s"
    return f"{secs}s"


def render_stage_wall_clock(entries: list[dict]) -> str:
    clocks = stage_wall_clock(entries)
    if not clocks:
        return "None recorded."
    lines = [
        "| stage | first ts | last ts | span | active time |",
        "|---|---|---|---|---|",
    ]
    for stage in sorted(clocks):
        c = clocks[stage]
        lines.append(
            f"| {stage} | {c['first'].isoformat()} | {c['last'].isoformat()} | "
            f"{fmt_duration(c['span_seconds'])} | {fmt_duration(c['active_seconds'])} |"
        )
    return "\n".join(lines)


def render_models_per_stage(entries: list[dict]) -> str:
    by_stage: dict[str, dict[str, int]] = {}
    for e in entries:
        stage = e.get("foundry_stage") or "unknown"
        model = e.get("model")
        model = model if isinstance(model, str) and model else "unknown"
        by_stage.setdefault(stage, {})
        by_stage[stage][model] = by_stage[stage].get(model, 0) + 1

    if not by_stage:
        return "None recorded."
    lines = ["| stage | model | count |", "|---|---|---:|"]
    for stage in sorted(by_stage):
        for model in sorted(by_stage[stage]):
            lines.append(f"| {stage} | {model} | {by_stage[stage][model]} |")
    return "\n".join(lines)


def read_human_interventions(root: Path) -> str:
    path = root / "provenance" / "human-interventions.md"
    if not path.is_file():
        return "`provenance/human-interventions.md` not found."
    return path.read_text(encoding="utf-8").strip()


def read_known_gaps(root: Path) -> str:
    path = root / "provenance" / "README.md"
    if not path.is_file():
        return "`provenance/README.md` not found."
    text = path.read_text(encoding="utf-8")
    lines = text.splitlines()
    start = None
    for i, line in enumerate(lines):
        if line.strip().lower() == "## known gaps":
            start = i
            break
    if start is None:
        return "No \"Known gaps\" section found in provenance/README.md."
    end = len(lines)
    for i in range(start + 1, len(lines)):
        if lines[i].startswith("## "):
            end = i
            break
    return "\n".join(lines[start + 1 : end]).strip()


def build_summary(log_path: Path, root: Path) -> str:
    entries, bad_lines = load_entries(log_path)

    rel_log = log_path
    try:
        rel_log = log_path.relative_to(root)
    except ValueError:
        pass

    parts = [
        "# provenance/SUMMARY.md",
        "",
        "Generated by `tools/cleanroom/summarize.py` (SPEC S5 M6). Do not "
        "hand-edit; re-run `make provenance-summary`.",
        "",
        f"Source: `{rel_log}` ({len(entries)} parsed line(s)).",
        "",
        "## Totals by tool",
        "",
        render_counts_table(totals_by(entries, "tool"), "tool"),
        "",
        "## Totals by stage",
        "",
        render_counts_table(totals_by(entries, "foundry_stage"), "stage"),
        "",
        "## Denied calls",
        "",
        render_denied_table(entries),
        "",
        "## Files read",
        "",
        render_files_read(entries, root),
        "",
        "## URLs fetched",
        "",
        render_urls(entries),
        "",
        "### Fetch scripts",
        "",
        render_fetch_scripts(entries, root),
        "",
        "## Per-stage wall clock",
        "",
        f"Active time sums inter-call gaps under {ACTIVE_GAP_MINUTES} minutes; "
        "a longer gap is treated as idle and excluded.",
        "",
        render_stage_wall_clock(entries),
        "",
        "## Models per stage",
        "",
        render_models_per_stage(entries),
        "",
        "## Human interventions",
        "",
        read_human_interventions(root),
        "",
        "## Known gaps",
        "",
        read_known_gaps(root),
        "",
        "## Parse errors",
        "",
        f"{bad_lines} line(s) failed to parse and were excluded from every "
        "count above (never dropped silently: this is the count).",
        "",
    ]
    return "\n".join(parts)


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--log", type=str, default=None)
    parser.add_argument("--out", type=str, default=None)
    args = parser.parse_args(argv)

    root = REPO_ROOT
    log_path = Path(args.log) if args.log else default_log_path(root)
    out_path = Path(args.out) if args.out else root / "provenance" / "SUMMARY.md"

    summary = build_summary(log_path, root)
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(summary, encoding="utf-8")
    print(f"summarize.py: wrote {out_path} from {log_path}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
