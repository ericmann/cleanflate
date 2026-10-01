import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
TOOL = HERE.parent / "asm_branches.py"
SAMPLE = HERE / "fixtures" / "sample.s"

sys.path.insert(0, str(HERE.parent))
import asm_branches  # noqa: E402


def rows_for(filters):
    functions = asm_branches.parse_functions(SAMPLE.read_text())
    return {r["name"]: r for r in asm_branches.select(functions, filters)}


class AsmBranchesTest(unittest.TestCase):
    def test_counts_on_fixture(self):
        rows = rows_for(["fast_loop", "checksum"])
        fast = rows["cleanflate::fast_loop::run"]
        # 16 instruction lines (labels, directives and the .size are not counted).
        self.assertEqual(fast["instructions"], 16)
        # jb and je are conditional; the unconditional jmp is not.
        self.assertEqual(fast["cond_branches"], 2)
        # Three calls to panic or bounds-failure routines, at two distinct
        # (target, location) sites; `helper` is not one of them.
        self.assertEqual(fast["panic_calls"], 3)
        self.assertEqual(fast["panic_sites"], 2)
        crc = rows["cleanflate::checksum::crc32"]
        self.assertEqual(crc["instructions"], 7)
        self.assertEqual(crc["cond_branches"], 2)
        self.assertEqual(crc["panic_calls"], 0)
        self.assertEqual(crc["panic_sites"], 0)

    def test_src_attribution_on_fixture(self):
        _, sources = asm_branches.parse(SAMPLE.read_text())
        rows = {r["name"]: r for r in asm_branches.select_sources(sources, [".rs"])}
        # The first four instructions of `run` and all of `crc32` carry a
        # fast_loop.rs `.loc`; the rest of `run` was inlined from window.rs.
        fl = rows["src:fast_loop.rs"]
        self.assertEqual((fl["instructions"], fl["cond_branches"], fl["panic_calls"]), (11, 3, 0))
        win = rows["src:window.rs"]
        self.assertEqual(
            (win["instructions"], win["cond_branches"], win["panic_calls"], win["panic_sites"]),
            (12, 1, 3, 2),
        )
        self.assertEqual(list(asm_branches.select_sources(sources, ["fast_loop"]))[0]["name"], "src:fast_loop.rs")
        self.assertEqual(asm_branches.select_sources(sources, ["nothing.rs"]), [])

    def test_src_flag_reports_rows(self):
        proc = subprocess.run(
            [sys.executable, str(TOOL), "--src", "window.rs", str(SAMPLE)],
            capture_output=True,
            text=True,
        )
        self.assertEqual(proc.returncode, 0, proc.stderr)
        self.assertIn("| src:window.rs | 12 | 1 | 3 | 2 |", proc.stdout)

    def test_func_filter_substring(self):
        self.assertEqual(list(rows_for(["crc32"])), ["cleanflate::checksum::crc32"])
        self.assertEqual(list(rows_for(["fast_loop::run"])), ["cleanflate::fast_loop::run"])
        self.assertEqual(rows_for(["no_such_function"]), {})
        # The mangled name matches too.
        self.assertEqual(list(rows_for(["h3333333333333333"])), ["cleanflate::checksum::crc32"])

    def test_json_shape(self):
        with tempfile.TemporaryDirectory() as d:
            out = Path(d) / "asm.json"
            proc = subprocess.run(
                [sys.executable, str(TOOL), "--func", "fast_loop", "--json", str(out), str(SAMPLE)],
                capture_output=True,
                text=True,
            )
            self.assertEqual(proc.returncode, 0, proc.stderr)
            self.assertIn("| cleanflate::fast_loop::run |", proc.stdout)
            data = json.loads(out.read_text())
            self.assertEqual(data["schema"], 1)
            self.assertEqual(data["kind"], "asm")
            self.assertIn("git_head", data)
            self.assertIn("rustc", data)
            self.assertEqual(
                sorted(data["functions"][0]),
                ["cond_branches", "instructions", "name", "panic_calls", "panic_sites"],
            )

    def test_missing_file_and_no_match_fail(self):
        for argv in (
            ["--func", "x", "/nonexistent/file.s"],
            ["--func", "no_such_function", str(SAMPLE)],
        ):
            proc = subprocess.run(
                [sys.executable, str(TOOL), *argv], capture_output=True, text=True
            )
            self.assertNotEqual(proc.returncode, 0)


if __name__ == "__main__":
    unittest.main()
