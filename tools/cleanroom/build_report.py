#!/usr/bin/env python3
"""Generate `docs/BUILD_REPORT.md` from git history and the docs (SPEC S5
M6, S6). Python 3 stdlib only. Usage: `build_report.py [--root PATH]
[--out PATH]`.
"""

from __future__ import annotations

import argparse
import re
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent.parent

RECORD_SEP = "\x1e"
FIELD_SEP = "\x1f"

TASK_RE = re.compile(r"\[task:(P(\d+)-\S+)\]")
STAGE_RE = re.compile(r"\[stage:(\w+)\]")
DEFECTS_RE = re.compile(r"^Defects-caught:\s*(.*)$", re.MULTILINE)


def git_log(root: Path) -> list[dict]:
    """Every commit as {sha, subject, body}, oldest first."""
    fmt = f"%H{FIELD_SEP}%s{FIELD_SEP}%b{RECORD_SEP}"
    try:
        out = subprocess.run(
            ["git", "log", "--reverse", f"--format={fmt}"],
            cwd=root,
            capture_output=True,
            text=True,
            check=True,
        ).stdout
    except Exception:
        return []
    commits = []
    for record in out.split(RECORD_SEP):
        record = record.strip("\n")
        if not record:
            continue
        parts = record.split(FIELD_SEP)
        if len(parts) < 3:
            continue
        sha, subject, body = parts[0], parts[1], parts[2]
        commits.append({"sha": sha, "subject": subject, "body": body})
    return commits


def commit_stage(commit: dict) -> str | None:
    m = STAGE_RE.search(commit["body"]) or STAGE_RE.search(commit["subject"])
    return m.group(1) if m else None


def commit_task(commit: dict) -> tuple[str, int] | None:
    """(task_id, phase_number), or None if this commit names no task."""
    m = TASK_RE.search(commit["body"]) or TASK_RE.search(commit["subject"])
    if not m:
        return None
    return m.group(1), int(m.group(2))


def milestones_section(commits: list[dict]) -> str:
    phase_tasks: dict[int, set[str]] = {}
    phase_commits: dict[int, int] = {}
    for c in commits:
        t = commit_task(c)
        if t is None:
            continue
        task_id, phase = t
        phase_tasks.setdefault(phase, set()).add(task_id)
        phase_commits[phase] = phase_commits.get(phase, 0) + 1

    if not phase_tasks:
        return "No `[task:P<n>-...]`-tagged commits found yet."

    lines = ["| phase | tasks | commits |", "|---|---:|---:|"]
    for phase in sorted(phase_tasks):
        lines.append(
            f"| P{phase} | {len(phase_tasks[phase])} | {phase_commits[phase]} |"
        )
    return "\n".join(lines)


DEFECT_KEYWORDS = (
    ("fuzz", "fuzz"),
    ("conformance", "conformance"),
    ("proptest", "tests"),
    ("test", "tests"),
)


def classify_defect(text: str, stage: str | None) -> str:
    """Commits tagged [stage:fix] are review-round fixes: whatever defect
    they describe was surfaced by the reviewer driving this round, so it
    counts as "review" regardless of what tool the fix's own commit body
    mentions. Everything else is classified by the first keyword its
    Defects-caught text matches."""
    if stage == "fix":
        return "review"
    low = text.lower()
    for keyword, category in DEFECT_KEYWORDS:
        if keyword in low:
            return category
    return "other"


def defects_section(commits: list[dict]) -> str:
    counts: dict[str, int] = {}
    entries: list[tuple[str, str, str]] = []  # (sha, category, text)
    for c in commits:
        m = DEFECTS_RE.search(c["body"])
        if not m:
            continue
        text = m.group(1).strip()
        if not text or text.lower().startswith("none"):
            continue
        stage = commit_stage(c)
        category = classify_defect(text, stage)
        counts[category] = counts.get(category, 0) + 1
        entries.append((c["sha"][:8], category, text))

    if not entries:
        return "No defects recorded (every `Defects-caught:` line so far says `none`)."

    lines = ["| category | count |", "|---|---:|"]
    for category in sorted(counts):
        lines.append(f"| {category} | {counts[category]} |")
    lines.append("")
    lines.append("| commit | category | defect |")
    lines.append("|---|---|---|")
    for sha, category, text in entries:
        cell = text.replace("|", "\\|").replace("\n", " ")
        if len(cell) > 300:
            cell = cell[:300] + "..."
        lines.append(f"| {sha} | {category} | {cell} |")
    return "\n".join(lines)


def review_rounds_section(commits: list[dict]) -> str:
    rounds = [c for c in commits if c["subject"].startswith("review:")]
    if not rounds:
        return (
            "Pending: regenerate with `make build-report` after the "
            "Foundry review converges."
        )
    lines = ["| commit | subject |", "|---|---|"]
    for c in rounds:
        lines.append(f"| {c['sha'][:8]} | {c['subject']} |")
    lines.append("")
    lines.append(f"{len(rounds)} review round(s) recorded.")
    return "\n".join(lines)


def read_perf_result_line(root: Path) -> str:
    """The `**Result:**` paragraph from docs/PERF.md, gathering every
    line until the next blank line (source markdown wraps it across
    several physical lines as ordinary prose)."""
    path = root / "docs" / "PERF.md"
    if not path.is_file():
        return "docs/PERF.md not found."
    lines = path.read_text(encoding="utf-8").splitlines()
    for i, line in enumerate(lines):
        if line.strip().startswith("**Result:**"):
            paragraph = [line.strip()]
            for cont in lines[i + 1 :]:
                if not cont.strip():
                    break
                paragraph.append(cont.strip())
            return " ".join(paragraph)
    return "No `**Result:**` line found in docs/PERF.md."


def read_provenance_gaps(root: Path) -> str:
    path = root / "provenance" / "README.md"
    if not path.is_file():
        return "provenance/README.md not found."
    lines = path.read_text(encoding="utf-8").splitlines()
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


def read_divergence_classes(root: Path) -> str:
    path = root / "fuzz" / "artifacts" / "divergences.jsonl"
    if not path.is_file():
        return "No divergence log present (fuzz-smoke/fuzz-long has not been run in this checkout)."
    classes: set[str] = set()
    import json

    with open(path, encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            try:
                obj = json.loads(line)
            except Exception:
                continue
            framing = obj.get("framing", "unknown")
            cls = obj.get("class", "unknown")
            classes.add(f"{framing}/{cls}")
    if not classes:
        return "Divergence log present but empty: zero divergence classes recorded."
    return "\n".join(f"- {c}" for c in sorted(classes))


def known_limitations_section(root: Path) -> str:
    parts = [
        "- **M5 deferred**: no_std support and a compressor are out of "
        "scope for this build (SPEC S7); only the decoder (inflate) and "
        "a C ABI are ever in scope, and the C ABI itself is also out of "
        "scope per SPEC S7.",
        "",
        f"- **Performance**: {read_perf_result_line(root)}",
        "",
        "- **Provenance gaps** (from `provenance/README.md`):",
        "",
        read_provenance_gaps(root),
        "",
        "- **Open divergence classes** (from `fuzz/artifacts/divergences.jsonl`, "
        "if present -- gitignored, so this section only has content in a "
        "checkout where fuzzing has actually run):",
        "",
        read_divergence_classes(root),
    ]
    return "\n".join(parts)


def build_report(root: Path) -> str:
    commits = git_log(root)
    parts = [
        "# docs/BUILD_REPORT.md",
        "",
        "Generated by `tools/cleanroom/build_report.py` (SPEC S5 M6, S6). "
        "Do not hand-edit; re-run `make build-report`.",
        "",
        "## Milestones",
        "",
        "Per phase: distinct `[task:P<n>-...]` ids and total commits "
        "naming a task in that phase, from `git log`.",
        "",
        milestones_section(commits),
        "",
        "## Defects caught",
        "",
        "Aggregated from every commit's `Defects-caught:` line, "
        "classified by keyword (fuzz/conformance/tests) except "
        "`[stage:fix]` commits, always counted as `review` (their defect "
        "was surfaced by a review round, not directly by a test run).",
        "",
        defects_section(commits),
        "",
        "## Review rounds",
        "",
        review_rounds_section(commits),
        "",
        "## Known limitations",
        "",
        known_limitations_section(root),
        "",
    ]
    return "\n".join(parts)


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=str, default=None)
    parser.add_argument("--out", type=str, default=None)
    args = parser.parse_args(argv)

    root = Path(args.root) if args.root else REPO_ROOT
    out_path = Path(args.out) if args.out else root / "docs" / "BUILD_REPORT.md"

    report = build_report(root)
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(report, encoding="utf-8")
    print(f"build_report.py: wrote {out_path}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
