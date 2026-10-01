#!/usr/bin/env python3
"""Count instructions, conditional branches and panic/bounds-failure call
sites per function in cleanflate's own generated assembly (SPEC 4.3,
"Bounds-check elision"). Input is the AT&T-syntax `.s` that
`cargo rustc --release -p cleanflate --lib -- --emit asm` writes; nothing
but cleanflate's own assembly is ever read.

Python 3 stdlib only. Usage:
    asm_branches.py --func SUBSTRING [--func ...] [--json PATH] FILE.s...

A function is reported when its mangled or demangled name contains any
`--func` substring. Per function:
  instructions   every instruction line (not labels, directives, comments)
  cond_branches  jump mnemonics other than an unconditional `jmp`
  panic_calls    call/jmp instructions whose target names a panic or
                 bounds-failure routine
  panic_sites    distinct (target, location) pairs among those: the location
                 is the last `.Lanon` operand seen in the eight instructions
                 before the call, which is how each source-level check loads
                 its own source location

`--src SUBSTRING` reports the same counters by *source file* instead of by
function: every instruction is attributed to the file of the most recent
`.loc` directive, so a function the optimiser inlined into another (the fast
loop is inlined into `Inflater::step_into` in the library build) is still
counted under its own file, and code inlined into it from other files is
counted under those files.
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from pathlib import Path

PANIC_MARKERS = (
    "panic",
    "panic_bounds_check",
    "slice_start_index_len_fail",
    "slice_end_index_len_fail",
    "slice_index_order_fail",
    "unwrap_failed",
    "expect_failed",
    "len_mismatch",
)

FILE_RE = re.compile(r'^\s*\.file\s+(\d+)\s+"([^"]*)"(?:\s+"([^"]*)")?')
LOC_RE = re.compile(r"^\s*\.loc\s+(\d+)\b")
LABEL_RE = re.compile(r"^([A-Za-z_$][^\s:]*):")
TYPE_RE = re.compile(r"^\s*\.type\s+([^,\s]+),\s*@function")
INSTR_RE = re.compile(r"^\s+([a-z][a-z0-9.]*)\b\s*(.*)$")
MANGLED_COMPONENT_RE = re.compile(r"(\d+)")


def demangle(name: str) -> str:
    """A small legacy-mangling demangler: `_ZN<len><id>...E` to `a::b::c`,
    dropping the trailing hash. Anything else is returned unchanged."""
    if not name.startswith("_ZN"):
        return name
    rest = name[3:]
    parts = []
    while rest and rest[0].isdigit():
        m = MANGLED_COMPONENT_RE.match(rest)
        if not m:
            break
        n = int(m.group(1))
        start = m.end()
        parts.append(rest[start : start + n])
        rest = rest[start + n :]
    if parts and re.fullmatch(r"h[0-9a-f]{16}", parts[-1]):
        parts.pop()
    return "::".join(p.replace("$LT$", "<").replace("$GT$", ">") for p in parts) or name


def is_panic_target(operand: str) -> bool:
    return any(marker in operand for marker in PANIC_MARKERS)


def _blank() -> dict:
    return {"instructions": 0, "cond_branches": 0, "panic_calls": 0, "sites": set()}


def _count(stats: dict, mnemonic: str, operand: str, recent: list[str]) -> None:
    stats["instructions"] += 1
    if mnemonic.startswith("j") and mnemonic != "jmp":
        stats["cond_branches"] += 1
    if mnemonic in ("call", "jmp", "callq") and is_panic_target(operand):
        stats["panic_calls"] += 1
        location = ""
        for prior in reversed(recent[-8:]):
            found = re.search(r"\.Lanon[\w.$]*", prior)
            if found:
                location = found.group(0)
                break
        stats["sites"].add((operand, location))


def parse(text: str) -> tuple[dict[str, dict], dict[str, dict]]:
    """Counters per function symbol and per source file (by `.loc`), each in
    order of first appearance."""
    functions: dict[str, dict] = {}
    sources: dict[str, dict] = {}
    files: dict[str, str] = {}
    function_symbols: set[str] = set()
    current: str | None = None
    current_file = ""
    recent: list[str] = []  # the last few instruction operands
    for line in text.splitlines():
        m = FILE_RE.match(line)
        if m:
            files[m.group(1)] = m.group(3) or m.group(2)
            continue
        m = LOC_RE.match(line)
        if m:
            current_file = files.get(m.group(1), "")
            continue
        m = TYPE_RE.match(line)
        if m:
            function_symbols.add(m.group(1))
            continue
        m = LABEL_RE.match(line)
        if m and not m.group(1).startswith((".L", "LBB", "LJTI")):
            name = m.group(1)
            if name in function_symbols:
                current = name
                functions.setdefault(name, _blank())
                recent = []
            continue
        if line.lstrip().startswith((".size",)) and current is not None:
            current = None
            continue
        if current is None:
            continue
        m = INSTR_RE.match(line)
        if not m:
            continue
        mnemonic, operand = m.group(1), m.group(2).split("#")[0].strip()
        _count(functions[current], mnemonic, operand, recent)
        if current_file:
            _count(sources.setdefault(current_file, _blank()), mnemonic, operand, recent)
        recent.append(operand)
        if len(recent) > 16:
            recent = recent[-16:]
    return functions, sources


def parse_functions(text: str) -> dict[str, dict]:
    """Map function symbol -> counters, in order of first appearance."""
    return parse(text)[0]


def _row(name: str, stats: dict) -> dict:
    return {
        "name": name,
        "instructions": stats["instructions"],
        "cond_branches": stats["cond_branches"],
        "panic_calls": stats["panic_calls"],
        "panic_sites": len(stats["sites"]),
    }


def select(functions: dict[str, dict], filters: list[str]) -> list[dict]:
    rows = []
    for symbol, stats in functions.items():
        readable = demangle(symbol)
        if any(f in symbol or f in readable for f in filters):
            rows.append(_row(readable, stats))
    return rows


def select_sources(sources: dict[str, dict], filters: list[str]) -> list[dict]:
    """Rows named `src:<file>` for every source file matching a filter."""
    return [_row(f"src:{name}", stats) for name, stats in sources.items() if any(f in name for f in filters)]


def run_cmd(args: list[str]) -> str:
    try:
        out = subprocess.run(args, capture_output=True, text=True, check=True)
        return out.stdout.strip()
    except (OSError, subprocess.CalledProcessError):
        return "unknown"


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--func", action="append", default=[], help="function name substring (repeatable)")
    ap.add_argument("--src", action="append", default=[], help="source file substring (repeatable)")
    ap.add_argument("--json", help="write the JSON report here")
    ap.add_argument("files", nargs="+", help=".s files of cleanflate's own assembly")
    args = ap.parse_args(argv)

    if not args.func and not args.src:
        ap.error("give at least one --func or --src")
    merged: dict[str, dict] = {}
    merged_src: dict[str, dict] = {}
    for f in args.files:
        path = Path(f)
        if not path.is_file():
            print(f"asm_branches: {f} does not exist; run `make asm` first", file=sys.stderr)
            return 1
        functions, sources = parse(path.read_text(errors="replace"))
        for symbol, stats in functions.items():
            merged.setdefault(symbol, stats)
        for name, stats in sources.items():
            merged_src.setdefault(name, stats)
    rows = select(merged, args.func) + select_sources(merged_src, args.src)
    if not rows:
        print(
            f"asm_branches: nothing matches --func {args.func} or --src {args.src}; a "
            "function the optimiser inlined has no symbol of its own (try --src FILE.rs)",
            file=sys.stderr,
        )
        return 1
    rows.sort(key=lambda r: (-r["instructions"], r["name"]))

    print("| function | instructions | cond branches | panic calls | panic sites |")
    print("|---|---:|---:|---:|---:|")
    for r in rows:
        print(
            f"| {r['name']} | {r['instructions']} | {r['cond_branches']} | "
            f"{r['panic_calls']} | {r['panic_sites']} |"
        )
    if args.json:
        report = {
            "schema": 1,
            "kind": "asm",
            "git_head": run_cmd(["git", "rev-parse", "HEAD"]),
            "rustc": run_cmd(["rustc", "--version"]),
            "functions": rows,
        }
        out = Path(args.json)
        out.parent.mkdir(parents=True, exist_ok=True)
        out.write_text(json.dumps(report, indent=2) + "\n")
        print(f"asm_branches: wrote {out}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
