#!/usr/bin/env python3
"""PreToolUse clean-room guard (SPEC S2.2, S2.3).

Denies (exit 2, stderr reason, logs a 'denied' entry) any tool call that
would read, fetch or search for existing DEFLATE/zlib implementation
source. Allows everything else silently (exit 0, no log -- the
PostToolUse logger logs allowed calls). Fails closed on internal errors.

Python 3 stdlib only.
"""

from __future__ import annotations

import os
import re
import shlex
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import provenance  # noqa: E402

FORBIDDEN_WORDS = ("zlib", "miniz", "flate2", "libdeflate", "libz-sys", "libz_sys", "zopfli")
FORBIDDEN_WORDS_PLUS = FORBIDDEN_WORDS + ("inflate", "deflate", "gzip", "puff")

READ_FAMILY = {
    "cat", "less", "more", "head", "tail", "grep", "egrep", "fgrep", "rg",
    "ag", "ack", "find", "fd", "locate", "ls", "tree", "sed", "awk", "bat",
    "strings", "xxd", "od", "hexdump", "file", "zcat", "zless", "zgrep",
    "zmore", "view", "vi", "vim", "nano", "emacs",
}

WRAPPERS = {"sudo", "env", "time", "command", "xargs"}

NETWORK_RE = re.compile(r"""(?:^|[\s;|&`'"])(curl|wget|gh)(?:\s|$)""")
GIT_CLONE_RE = re.compile(r"\bgit\s+clone\b")
CARGO_VENDOR_RE = re.compile(r"\bcargo\s+(vendor|download)\b")
TARGET_BUILD_RE = re.compile(r"target/(\S*/)?build(/|\s|$)")
URL_RE = re.compile(r"https?://\S+")
ASSIGNMENT_RE = re.compile(r"^[A-Za-z_][A-Za-z0-9_]*=")


def word_hit(text: str, words) -> bool:
    if not text:
        return False
    low = text.lower()
    return any(w in low for w in words)


def canonicalize(path: str) -> str:
    """Resolve symlinks so a repo reachable through more than one mount
    path (e.g. a bind mount or symlinked workspace) still compares equal.
    Falls back to normpath if realpath fails."""
    try:
        return os.path.normpath(os.path.realpath(path))
    except Exception:
        return os.path.normpath(path)


def resolve_variants(raw: str, cwd: str):
    if not raw:
        raw = cwd or os.getcwd()
    expanded = os.path.expanduser(raw)
    if not os.path.isabs(expanded):
        base = cwd or os.getcwd()
        expanded = os.path.join(base, expanded)
    normed = os.path.normpath(expanded)
    variants = {normed}
    try:
        real = os.path.realpath(normed)
        variants.add(real)
    except Exception:
        pass
    return list(variants)


def is_under(path: str, base: str) -> bool:
    path_n = os.path.normpath(path)
    base_n = os.path.normpath(base)
    return path_n == base_n or path_n.startswith(base_n + os.sep)


def path_in_forbidden_roots(path: str, home: str, cargo_home: str) -> bool:
    roots = []
    if home:
        roots.append(os.path.normpath(os.path.join(home, ".cargo", "registry")))
        roots.append(os.path.normpath(os.path.join(home, ".cargo", "git")))
    if cargo_home:
        roots.append(os.path.normpath(os.path.join(cargo_home, "registry")))
    for r in roots:
        if is_under(path, r):
            return True
    segs = os.path.normpath(path).split(os.sep)
    for i, s in enumerate(segs):
        if s == "target":
            rest = segs[i + 1 :]
            if "build" in rest:
                return True
            if len(rest) >= 2 and rest[0] == "doc" and rest[1] == "src":
                return True
    return False


def in_exempt(path: str, root: str) -> bool:
    for sub in ("cleanflate", "oracle"):
        e = os.path.normpath(os.path.join(root, "crates", sub))
        if is_under(path, e):
            return True
    return False


def load_allowlist(root: str):
    path = os.path.join(root, "tools", "cleanroom", "allowlist.txt")
    urls = set()
    try:
        with open(path, "r", encoding="utf-8") as f:
            for line in f:
                line = line.strip()
                if not line or line.startswith("#"):
                    continue
                urls.add(line)
    except Exception:
        pass
    return urls


def lockdown_path(root: str) -> str:
    return os.environ.get("CLEANROOM_LOCKDOWN_FILE") or os.path.join(
        root, "tools", "cleanroom", "LOCKDOWN"
    )


def check_r3(tool_name: str, tool_input: dict, cwd: str, root: str, home: str, cargo_home: str):
    if tool_name == "Read":
        raw_path = tool_input.get("file_path", "") or ""
        variants = resolve_variants(raw_path, cwd)
        for v in variants:
            if path_in_forbidden_roots(v, home, cargo_home):
                return f"forbidden root: {v}"
        for v in variants:
            if word_hit(v, FORBIDDEN_WORDS) and not in_exempt(v, root):
                return f"forbidden word in path: {v}"
        for v in variants:
            if not is_under(v, root) and word_hit(v, FORBIDDEN_WORDS_PLUS):
                return f"forbidden word outside repo: {v}"
        return None

    if tool_name in ("Glob", "Grep"):
        raw_path = tool_input.get("path") or cwd or os.getcwd()
        variants = resolve_variants(raw_path, cwd)
        for v in variants:
            if path_in_forbidden_roots(v, home, cargo_home):
                return f"forbidden root: {v}"
        for v in variants:
            if word_hit(v, FORBIDDEN_WORDS) and not in_exempt(v, root):
                return f"forbidden word in path: {v}"
        for v in variants:
            if not is_under(v, root) and word_hit(v, FORBIDDEN_WORDS_PLUS):
                return f"forbidden word outside repo: {v}"
        field = "pattern" if tool_name == "Glob" else "glob"
        val = tool_input.get(field, "") or ""
        exempt_base = any(in_exempt(v, root) for v in variants)
        if word_hit(val, FORBIDDEN_WORDS) and not exempt_base:
            return f"forbidden word in {field}: {val}"
        return None

    return None


def _command_word(segment: str):
    toks = segment.strip().split()
    i = 0
    while i < len(toks):
        t = toks[i]
        if ASSIGNMENT_RE.match(t):
            i += 1
            continue
        base = os.path.basename(t)
        if base in WRAPPERS:
            i += 1
            while i < len(toks) and toks[i].startswith("-"):
                i += 1
            continue
        return t
    return None


def _is_path_like(tok: str, cwd: str) -> bool:
    if tok.startswith(("/", "~", "./", "../")):
        return True
    if "/" in tok:
        return True
    try:
        candidate = os.path.join(cwd or os.getcwd(), tok)
        if os.path.exists(candidate):
            return True
    except Exception:
        pass
    return False


def _check_read_family_segment(segment: str, cwd: str, root: str):
    cmd_word = _command_word(segment)
    if not cmd_word:
        return None
    base = os.path.basename(cmd_word)
    if base not in READ_FAMILY:
        return None

    try:
        toks = shlex.split(segment)
    except ValueError:
        return "unparseable command"

    path_tokens = [t for t in toks[1:] if _is_path_like(t, cwd)]
    non_path_tokens = [t for t in toks[1:] if not _is_path_like(t, cwd)]

    for t in path_tokens:
        variants = resolve_variants(t, cwd)
        for v in variants:
            if word_hit(v, FORBIDDEN_WORDS) and not in_exempt(v, root):
                return f"forbidden word in path argument: {t}"

    if word_hit(" ".join(non_path_tokens), FORBIDDEN_WORDS):
        bases = path_tokens if path_tokens else [cwd or os.getcwd()]
        target_dir = os.path.join(root, "target")
        fuzz_target_dir = os.path.join(root, "fuzz", "target")
        all_inside = True
        for t in bases:
            variants = resolve_variants(t, cwd)
            # A token counts as "inside root, not under target" if any of
            # its resolutions (raw normpath or realpath) does -- a
            # symlinked checkout can make one variant look external while
            # the canonical one is fine.
            token_ok = any(
                is_under(v, root) and not (is_under(v, target_dir) or is_under(v, fuzz_target_dir))
                for v in variants
            )
            if not token_ok:
                all_inside = False
        if not all_inside:
            return f"forbidden word in command outside repo/target: {segment}"

    if base in ("grep", "egrep", "fgrep"):
        recursive = False
        for t in toks[1:]:
            if t in ("-r", "-R", "--recursive"):
                recursive = True
            elif (
                t.startswith("-")
                and not t.startswith("--")
                and re.search(r"[rR]", t[1:])
            ):
                recursive = True
        if recursive:
            has_exclude = any(t.startswith("--exclude-dir=target") for t in toks)
            path_args = path_tokens if path_tokens else [cwd or os.getcwd()]
            root_target = os.path.join(root, "target")
            for t in path_args:
                for v in resolve_variants(t, cwd):
                    vn = os.path.normpath(v)
                    if vn == os.path.normpath(root) or is_under(root_target, vn):
                        if not has_exclude:
                            return "recursive grep at repo root without --exclude-dir=target"
    return None


def split_top_level_segments(command: str):
    """Split a shell command into segments on unquoted ; & && | ||  and
    newlines, so a literal '|' inside a quoted regex (e.g. grep -E "a|b")
    is never mistaken for a shell pipe."""
    segments = []
    current = []
    i = 0
    n = len(command)
    in_squote = False
    in_dquote = False
    while i < n:
        c = command[i]
        if in_squote:
            current.append(c)
            if c == "'":
                in_squote = False
            i += 1
            continue
        if in_dquote:
            current.append(c)
            if c == "\\" and i + 1 < n:
                current.append(command[i + 1])
                i += 2
                continue
            if c == '"':
                in_dquote = False
            i += 1
            continue
        if c == "'":
            in_squote = True
            current.append(c)
            i += 1
            continue
        if c == '"':
            in_dquote = True
            current.append(c)
            i += 1
            continue
        if c == "\\" and i + 1 < n:
            current.append(c)
            current.append(command[i + 1])
            i += 2
            continue
        if c in ";\n":
            segments.append("".join(current))
            current = []
            i += 1
            continue
        if c == "&":
            if i + 1 < n and command[i + 1] == "&":
                segments.append("".join(current))
                current = []
                i += 2
                continue
            segments.append("".join(current))
            current = []
            i += 1
            continue
        if c == "|":
            if i + 1 < n and command[i + 1] == "|":
                segments.append("".join(current))
                current = []
                i += 2
                continue
            segments.append("".join(current))
            current = []
            i += 1
            continue
        current.append(c)
        i += 1
    segments.append("".join(current))
    return segments


def check_bash(command: str, cwd: str, root: str, lockdown_exists: bool, allowlist):
    if GIT_CLONE_RE.search(command):
        return "git clone is forbidden (CR3)"
    if CARGO_VENDOR_RE.search(command):
        return "cargo vendor/download is forbidden (CR3)"

    m = NETWORK_RE.search(command)
    if m:
        word = m.group(1)
        if word == "gh":
            return "gh is forbidden (CR3)"
        if lockdown_exists:
            return f"{word} denied: post-M0 lockdown"
        urls = URL_RE.findall(command)
        cleaned = [u.strip("'\"`)>;,") for u in urls]
        if not cleaned:
            return f"{word} with no recognizable URL"
        for u in cleaned:
            if u not in allowlist:
                return f"{word} url not allowlisted: {u}"

    if lockdown_exists and (
        "fetch-rfcs.sh" in command
        or "fetch-corpora.sh" in command
        or "CLEANROOM_ALLOW_CORPUS_FETCH" in command
    ):
        return "fetch script denied: post-M0 lockdown"

    if (
        ".cargo/registry" in command
        or ".cargo/git" in command
        or "target/doc/src" in command
        or TARGET_BUILD_RE.search(command)
    ):
        return "forbidden root referenced in command"

    for segment in split_top_level_segments(command):
        segment = segment.strip()
        if not segment:
            continue
        reason = _check_read_family_segment(segment, cwd, root)
        if reason:
            return reason

    return None


def evaluate(hook_input: dict):
    """Return (decision, reason) for one hook payload. decision is
    'allowed' or 'denied'; reason is a string when denied, else None."""
    tool_name = hook_input.get("tool_name", "") if isinstance(hook_input, dict) else ""
    tool_input = hook_input.get("tool_input", {}) if isinstance(hook_input, dict) else {}
    if not isinstance(tool_input, dict):
        tool_input = {}
    cwd = hook_input.get("cwd") or os.getcwd() if isinstance(hook_input, dict) else os.getcwd()
    root = provenance.repo_root(hook_input if isinstance(hook_input, dict) else {})
    root = canonicalize(root)
    home = os.environ.get("HOME", "")
    cargo_home = os.environ.get("CARGO_HOME")
    lockdown_exists = os.path.exists(lockdown_path(root))
    allowlist = load_allowlist(root)

    if tool_name == "WebSearch":
        return "denied", "WebSearch is forbidden (SPEC 2.3)"

    if tool_name == "WebFetch":
        url = (tool_input.get("url") or "").strip()
        if lockdown_exists:
            return "denied", "WebFetch denied: post-M0 lockdown"
        if url in allowlist:
            return "allowed", None
        return "denied", f"WebFetch url not in allowlist: {url}"

    if tool_name in ("Read", "Glob", "Grep"):
        reason = check_r3(tool_name, tool_input, cwd, root, home, cargo_home)
        if reason:
            return "denied", reason
        return "allowed", None

    if tool_name == "Bash":
        command = tool_input.get("command", "") or ""
        reason = check_bash(command, cwd, root, lockdown_exists, allowlist)
        if reason:
            return "denied", reason
        return "allowed", None

    return "allowed", None


def main() -> int:
    hook_input: dict = {}
    try:
        hook_input = provenance.read_hook_input()
        decision, reason = evaluate(hook_input)
    except Exception as exc:  # noqa: BLE001 -- fail closed (R5)
        decision, reason = "denied", f"guard internal error: {exc}"

    if decision == "denied":
        try:
            root = provenance.repo_root(hook_input)
            path = provenance.log_path(root)
            ctx = provenance.context(hook_input, root)
            tool_name = hook_input.get("tool_name", "") if isinstance(hook_input, dict) else ""
            tool_input = hook_input.get("tool_input", {}) if isinstance(hook_input, dict) else {}
            entry = {
                "ts": provenance.now_iso(),
                "foundry_stage": ctx["foundry_stage"],
                "task_id": ctx["task_id"],
                "model": ctx["model"],
                "tool": tool_name,
                "target": provenance.target_of(tool_name, tool_input),
                "decision": "denied",
                "reason": reason,
            }
            provenance.append(entry, path)
        except Exception:
            pass
        sys.stderr.write(f"cleanroom guard: {reason}\n")
        return 2

    return 0


if __name__ == "__main__":
    sys.exit(main())
