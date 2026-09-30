import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

import build_report  # noqa: E402


def run(cmd, cwd):
    subprocess.run(cmd, cwd=cwd, check=True, capture_output=True, text=True)


class BuildReportTestBase(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
        run(["git", "init", "-q"], self.root)
        run(["git", "config", "user.email", "test@example.com"], self.root)
        run(["git", "config", "user.name", "Test"], self.root)
        (self.root / "docs").mkdir()
        (self.root / "provenance").mkdir()
        (self.root / "provenance" / "README.md").write_text(
            "## Known gaps\n\n- gap A\n- gap B\n"
        )
        (self.root / "docs" / "PERF.md").write_text(
            "some numbers\n\n**Result:** cleanflate / C zlib = 42% on "
            "Silesia (target 70%): NOT MET\n"
        )

    def tearDown(self):
        self.tmp.cleanup()

    def commit(self, message):
        (self.root / "README.md").write_text(message)
        run(["git", "add", "-A"], self.root)
        run(["git", "commit", "-q", "-m", message], self.root)

    def report(self):
        return build_report.build_report(self.root)


class TestCountsTasksPerPhase(BuildReportTestBase):
    def test_counts_tasks_per_phase(self):
        self.commit("P0-01: scaffold\n\n[stage:implement] [task:P0-01]\n\nDefects-caught: none")
        self.commit("P0-02: checks\n\n[stage:implement] [task:P0-02]\n\nDefects-caught: none")
        self.commit("P1-01: inputs\n\n[stage:implement] [task:P1-01]\n\nDefects-caught: none")
        # A second commit against the same task (e.g. a fix round) should
        # count towards phase 0's commit total but not inflate its task
        # count past the 2 distinct P0 tasks above.
        self.commit("P0-01: scaffold fixup\n\n[stage:fix] [task:P0-01]\n\nDefects-caught: none")

        text = self.report()
        section = text.split("## Milestones")[1].split("## Defects caught")[0]
        self.assertIn("| P0 | 2 | 3 |", section)
        self.assertIn("| P1 | 1 | 1 |", section)


class TestDefectsClassified(BuildReportTestBase):
    def test_defects_classified(self):
        self.commit(
            "P1-04: checksums\n\n[stage:implement] [task:P1-04]\n\n"
            "Defects-caught: a checksum test caught an off-by-one"
        )
        self.commit(
            "P2-05: conformance\n\n[stage:implement] [task:P2-05]\n\n"
            "Defects-caught: the conformance corpus found a bad symbol"
        )
        self.commit(
            "P4-01: fuzz target\n\n[stage:implement] [task:P4-01]\n\n"
            "Defects-caught: fuzzing found a panic on empty input"
        )
        self.commit(
            "P2-06: acceptance\n\n[stage:fix] [task:P2-06]\n\n"
            "Defects-caught: a test caught this, but it's a fix-round commit"
        )
        self.commit(
            "P3-01: noop\n\n[stage:implement] [task:P3-01]\n\n"
            "Defects-caught: none"
        )

        text = self.report()
        section = text.split("## Defects caught")[1].split("## Review rounds")[0]
        self.assertIn("| tests | 1 |", section)
        self.assertIn("| conformance | 1 |", section)
        self.assertIn("| fuzz | 1 |", section)
        # [stage:fix] always counts as review, even though its own text
        # mentions "test".
        self.assertIn("| review | 1 |", section)
        # The "none" entry must not appear at all.
        self.assertNotIn("none", section.lower())


class TestReviewPendingWhenNoReviewCommits(BuildReportTestBase):
    def test_review_pending_when_no_review_commits(self):
        self.commit("P0-01: scaffold\n\n[stage:implement] [task:P0-01]\n\nDefects-caught: none")

        text = self.report()
        section = text.split("## Review rounds")[1].split("## Known limitations")[0]
        self.assertIn("Pending: regenerate with `make build-report`", section)

    def test_review_rounds_counted_when_present(self):
        self.commit("P0-01: scaffold\n\n[stage:implement] [task:P0-01]\n\nDefects-caught: none")
        self.commit("review: round 1\n\n[stage:review]\n\nVerdict: CHANGES REQUESTED")

        text = self.report()
        section = text.split("## Review rounds")[1].split("## Known limitations")[0]
        self.assertIn("review: round 1", section)
        self.assertIn("1 review round(s) recorded.", section)


class TestKnownLimitationsIncludePerfResult(BuildReportTestBase):
    def test_known_limitations_include_perf_result(self):
        self.commit("P0-01: scaffold\n\n[stage:implement] [task:P0-01]\n\nDefects-caught: none")

        text = self.report()
        section = text.split("## Known limitations")[1]
        self.assertIn("cleanflate / C zlib = 42% on", section)
        self.assertIn("NOT MET", section)
        self.assertIn("M5 deferred", section)
        self.assertIn("gap A", section)
        self.assertIn("gap B", section)


class TestSinceLimitsCommits(BuildReportTestBase):
    def test_since_limits_commits(self):
        self.commit("P0-01: old\n\n[stage:implement] [task:P0-01]\n\nDefects-caught: none")
        self.commit("P0-02: old\n\n[stage:implement] [task:P0-02]\n\nDefects-caught: none")
        base = subprocess.run(
            ["git", "rev-parse", "HEAD"], cwd=self.root, check=True,
            capture_output=True, text=True,
        ).stdout.strip()
        self.commit("P1-01: new\n\n[stage:implement] [task:P1-01]\n\nDefects-caught: none")
        full = build_report.build_report(self.root)
        scoped = build_report.build_report(self.root, since=base)
        self.assertIn("| P0 | 2 | 2 |", full)
        self.assertIn("| P1 | 1 | 1 |", full)
        self.assertNotIn("| P0 |", scoped)
        self.assertIn("| P1 | 1 | 1 |", scoped)
        self.assertIn(f"Range: commits in `{base}..HEAD` (1 commit(s)).", scoped)


class TestReadsLastResultLine(BuildReportTestBase):
    def test_reads_last_result_line(self):
        (self.root / "docs" / "PERF.md").write_text(
            "phase one\n\n**Result:** phase one: 25% NOT MET\n\n"
            "phase two text\n\n**Result:** phase two: 66% NOT MET,\n"
            "second physical line\n\ntrailer\n"
        )
        line = build_report.read_perf_result_line(self.root)
        self.assertEqual(
            line, "**Result:** phase two: 66% NOT MET, second physical line"
        )


if __name__ == "__main__":
    unittest.main()
