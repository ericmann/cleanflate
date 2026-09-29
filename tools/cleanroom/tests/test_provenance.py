import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

import provenance  # noqa: E402

CLEANROOM_DIR = Path(__file__).resolve().parent.parent


class TestTargetOf(unittest.TestCase):
    def test_target_for_each_tool(self):
        self.assertEqual(
            provenance.target_of("Read", {"file_path": "/a/b.rs"}), "/a/b.rs"
        )
        self.assertEqual(
            provenance.target_of("Write", {"file_path": "/a/b.rs"}), "/a/b.rs"
        )
        self.assertEqual(
            provenance.target_of("Edit", {"file_path": "/a/b.rs"}), "/a/b.rs"
        )
        self.assertEqual(
            provenance.target_of("NotebookEdit", {"notebook_path": "/a/n.ipynb"}),
            "/a/n.ipynb",
        )
        self.assertEqual(
            provenance.target_of(
                "Grep", {"path": "/a", "glob": "*.rs", "pattern": "foo"}
            ),
            "path=/a glob=*.rs pattern=foo",
        )
        self.assertEqual(
            provenance.target_of("Glob", {"path": "/a", "pattern": "*.rs"}),
            "path=/a pattern=*.rs",
        )
        self.assertEqual(
            provenance.target_of("WebFetch", {"url": "https://example.com"}),
            "https://example.com",
        )
        self.assertEqual(
            provenance.target_of("WebSearch", {"query": "rfc 1951"}), "rfc 1951"
        )
        self.assertEqual(
            provenance.target_of("Bash", {"command": "echo hi"}), "echo hi"
        )
        long_cmd = "x" * 5000
        self.assertEqual(
            len(provenance.target_of("Bash", {"command": long_cmd})), 4000
        )
        self.assertEqual(
            provenance.target_of(
                "Task", {"subagent_type": "implementer", "description": "do thing"}
            ),
            "implementer: do thing",
        )
        other = provenance.target_of("SomeTool", {"x": 1})
        self.assertTrue(other.startswith("SomeTool "))


class TestStageDerivation(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = self.tmp.name
        self._env_backup = dict(os.environ)

    def tearDown(self):
        os.environ.clear()
        os.environ.update(self._env_backup)
        self.tmp.cleanup()

    def test_stage_from_agent_type(self):
        os.environ.pop("FOUNDRY_STAGE", None)
        self.assertEqual(
            provenance.derive_stage({"agent_type": "planner"}, self.root), "plan"
        )
        self.assertEqual(
            provenance.derive_stage({"agent_type": "implementer"}, self.root),
            "implement",
        )
        self.assertEqual(
            provenance.derive_stage({"agent_type": "reviewer"}, self.root), "review"
        )
        self.assertEqual(
            provenance.derive_stage({"agent_type": "summarizer"}, self.root),
            "summarize",
        )

    def test_stage_from_env(self):
        os.environ["FOUNDRY_STAGE"] = "review"
        self.assertEqual(provenance.derive_stage({}, self.root), "review")
        os.environ["FOUNDRY_STAGE"] = "not-a-stage"
        self.assertIsNone(provenance.derive_stage({}, self.root))

    def test_stage_from_lock_and_round(self):
        os.environ.pop("FOUNDRY_STAGE", None)
        foundry_dir = Path(self.root) / ".foundry"
        foundry_dir.mkdir()
        (foundry_dir / "implement.lock").write_text("")
        self.assertEqual(provenance.derive_stage({}, self.root), "implement")
        (foundry_dir / "state.json").write_text(json.dumps({"round": 1}))
        self.assertEqual(provenance.derive_stage({}, self.root), "fix")

    def test_stage_null_outside_flight(self):
        os.environ.pop("FOUNDRY_STAGE", None)
        self.assertIsNone(provenance.derive_stage({}, self.root))


class TestTaskId(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = self.tmp.name
        self._env_backup = dict(os.environ)

    def tearDown(self):
        os.environ.clear()
        os.environ.update(self._env_backup)
        self.tmp.cleanup()

    def test_task_id_from_progress(self):
        os.environ.pop("FOUNDRY_TASK_ID", None)
        docs = Path(self.root) / "docs"
        docs.mkdir()
        (docs / "PROGRESS.md").write_text(
            "# progress\n\n## Tasks\n- [x] P0-01: done\n- [~] P0-03: in progress\n- [ ] P0-04: todo\n"
        )
        self.assertEqual(
            provenance.derive_task_id(self.root, "implement"), "P0-03:"
        )

    def test_task_id_env_override(self):
        os.environ["FOUNDRY_TASK_ID"] = "P9-99"
        self.assertEqual(
            provenance.derive_task_id(self.root, "implement"), "P9-99"
        )


class TestModelDerivation(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self._env_backup = dict(os.environ)

    def tearDown(self):
        os.environ.clear()
        os.environ.update(self._env_backup)
        self.tmp.cleanup()

    def test_model_from_transcript_tail(self):
        os.environ.pop("CLEANROOM_MODEL", None)
        transcript = Path(self.tmp.name) / "transcript.jsonl"
        lines = [
            json.dumps({"message": {"model": "claude-old"}}),
            json.dumps({"message": {"model": "claude-newest"}}),
        ]
        transcript.write_text("\n".join(lines) + "\n")
        model = provenance.derive_model({"transcript_path": str(transcript)})
        self.assertEqual(model, "claude-newest")

    def test_model_null_when_unknown(self):
        os.environ.pop("CLEANROOM_MODEL", None)
        self.assertIsNone(provenance.derive_model({}))
        self.assertIsNone(
            provenance.derive_model({"transcript_path": "/nonexistent/path.jsonl"})
        )


class TestAppend(unittest.TestCase):
    def test_append_writes_one_json_line_with_spec_keys(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = os.path.join(tmp, "provenance", "tool-log.jsonl")
            entry = {
                "ts": "2026-01-01T00:00:00.000Z",
                "foundry_stage": "implement",
                "task_id": "P0-03",
                "model": None,
                "tool": "Read",
                "target": "/a/b",
                "decision": "allowed",
                "reason": None,
            }
            provenance.append(entry, path)
            with open(path, "r", encoding="utf-8") as f:
                lines = f.readlines()
            self.assertEqual(len(lines), 1)
            parsed = json.loads(lines[0])
            self.assertEqual(list(parsed.keys())[:8], list(provenance.ENTRY_KEYS))


class TestLogPy(unittest.TestCase):
    def test_log_py_exits_zero_on_garbage_stdin(self):
        with tempfile.TemporaryDirectory() as tmp:
            log_path = os.path.join(tmp, "tool-log.jsonl")
            env = dict(os.environ)
            env["CLEANROOM_LOG"] = log_path
            env["CLEANROOM_REPO_ROOT"] = tmp
            result = subprocess.run(
                [sys.executable, str(CLEANROOM_DIR / "log.py")],
                input="not json at all {{{",
                capture_output=True,
                text=True,
                env=env,
                timeout=10,
            )
            self.assertEqual(result.returncode, 0)


if __name__ == "__main__":
    unittest.main()
