import json
import os
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

import summarize  # noqa: E402

CLEANROOM_DIR = Path(__file__).resolve().parent.parent
FIXTURE = CLEANROOM_DIR / "tests" / "fixtures" / "tool-log.sample.jsonl"


class SummarizeTestBase(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
        (self.root / "provenance").mkdir(parents=True)
        (self.root / "crates" / "cleanflate" / "src").mkdir(parents=True)
        (self.root / "docs").mkdir(parents=True)

        (self.root / "provenance" / "human-interventions.md").write_text(
            "| when (UTC) | who | stage/task | what | why |\n"
            "|------------|-----|------------|------|-----|\n"
            "| 2026-01-01T00:00Z | operator | implement/P0-08 | did a thing | because |\n"
        )
        (self.root / "provenance" / "README.md").write_text(
            "# provenance\n\n"
            "## Entry schema\n\nsome text\n\n"
            "## Known gaps\n\n"
            "- gap one\n"
            "- gap two\n\n"
            "## Harness status during build\n\nLIVE\n"
        )

        rendered = FIXTURE.read_text().replace("__ROOT__", str(self.root))
        self.log_path = self.root / "provenance" / "tool-log.jsonl"
        self.log_path.write_text(rendered)

    def tearDown(self):
        self.tmp.cleanup()

    def summary(self):
        return summarize.build_summary(self.log_path, self.root)


class TestTotals(SummarizeTestBase):
    def test_totals(self):
        text = self.summary()
        self.assertIn("## Totals by tool", text)
        self.assertIn("## Totals by stage", text)
        # 7 implement-stage lines + 2 review-stage lines = 9 parsed entries
        # (one line in the fixture is deliberately invalid JSON).
        self.assertIn("| **total** | **9** |", text)
        self.assertIn("| implement | 7 |", text)
        self.assertIn("| review | 2 |", text)


class TestDeniedTable(SummarizeTestBase):
    def test_denied_table(self):
        text = self.summary()
        self.assertIn("## Denied calls", text)
        self.assertIn("forbidden root: /etc/passwd", text)
        self.assertIn("WebSearch is forbidden (SPEC 2.3)", text)
        # An allowed call must not show up in the denied table's reasons.
        self.assertNotIn("cargo build", text.split("## Denied calls")[1].split("## ")[0])


class TestFilesGrouped(SummarizeTestBase):
    def test_files_grouped_and_outside_repo_flagged(self):
        text = self.summary()
        section = text.split("## Files read")[1].split("## URLs fetched")[0]
        self.assertIn("`crates/` (1 file(s))", section)
        self.assertIn("crates/cleanflate/src/lib.rs", section)
        self.assertIn("`docs/` (1 file(s))", section)
        self.assertIn("### Outside repository", section)
        self.assertIn("/tmp/scratch-notes.txt", section)


class TestUrlsListed(SummarizeTestBase):
    def test_urls_listed(self):
        text = self.summary()
        section = text.split("## URLs fetched")[1].split("## Per-stage wall clock")[0]
        self.assertIn("https://www.rfc-editor.org/rfc/rfc1951.txt", section)


class TestStageWallClockActiveTime(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
        (self.root / "provenance").mkdir(parents=True)
        (self.root / "provenance" / "human-interventions.md").write_text("no rows\n")
        (self.root / "provenance" / "README.md").write_text("## Known gaps\n\nnone\n")

    def tearDown(self):
        self.tmp.cleanup()

    def entry(self, ts, stage="implement"):
        return {
            "ts": ts,
            "foundry_stage": stage,
            "task_id": "P0-01",
            "model": "m",
            "tool": "Bash",
            "target": "x",
            "decision": "allowed",
            "reason": None,
        }

    def test_stage_wall_clock_active_time(self):
        # Two calls 10s apart (active), then a 20-minute idle gap, then one
        # more call: span = 20m10s, but active time only counts the 10s
        # gap (the 20-minute gap is >= the 15-minute active threshold).
        entries = [
            self.entry("2026-01-01T00:00:00.000Z"),
            self.entry("2026-01-01T00:00:10.000Z"),
            self.entry("2026-01-01T00:20:10.000Z"),
        ]
        log_path = self.root / "provenance" / "tool-log.jsonl"
        log_path.write_text("\n".join(json.dumps(e) for e in entries) + "\n")

        text = summarize.build_summary(log_path, self.root)
        section = text.split("## Per-stage wall clock")[1].split("## Models per stage")[0]
        self.assertIn("implement", section)
        self.assertIn("20m10s", section)  # span
        self.assertIn("10s", section)  # active time (appears within a cell)
        # The 20-minute gap must not be counted as active.
        self.assertNotIn("20m20s", section)


class TestModelsUnknownForNull(SummarizeTestBase):
    def test_models_unknown_for_null(self):
        text = self.summary()
        section = text.split("## Models per stage")[1].split("## Human interventions")[0]
        self.assertIn("| implement | model-a | 6 |", section)
        self.assertIn("| implement | unknown | 1 |", section)
        self.assertIn("| review | model-b | 1 |", section)
        self.assertIn("| review | unknown | 1 |", section)


class TestBadLinesCounted(SummarizeTestBase):
    def test_bad_lines_counted(self):
        text = self.summary()
        self.assertIn("## Parse errors", text)
        self.assertIn("1 line(s) failed to parse", text)


class TestIncludesHumanInterventions(SummarizeTestBase):
    def test_includes_human_interventions(self):
        text = self.summary()
        section = text.split("## Human interventions")[1].split("## Known gaps")[0]
        self.assertIn("did a thing", section)
        self.assertIn("operator", section)

        gaps_section = text.split("## Known gaps")[1].split("## Parse errors")[0]
        self.assertIn("gap one", gaps_section)
        self.assertIn("gap two", gaps_section)


if __name__ == "__main__":
    unittest.main()
