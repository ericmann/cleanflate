import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

import guard  # noqa: E402
import provenance  # noqa: E402

CLEANROOM_DIR = Path(__file__).resolve().parent.parent

ALLOWLIST_TEXT = """\
https://www.rfc-editor.org/rfc/rfc1950.txt
https://www.rfc-editor.org/rfc/rfc1951.txt
https://www.rfc-editor.org/rfc/rfc1952.txt
https://rfc-editor.org/rfc/rfc1950.txt
https://rfc-editor.org/rfc/rfc1951.txt
https://rfc-editor.org/rfc/rfc1952.txt
https://corpus.canterbury.ac.nz/resources/cantrbry.tar.gz
https://sun.aei.polsl.pl/~sdeor/corpus/silesia.zip
https://sun.aei.polsl.pl//~sdeor/corpus/silesia.zip
http://sun.aei.polsl.pl/~sdeor/corpus/silesia.zip
http://sun.aei.polsl.pl//~sdeor/corpus/silesia.zip
"""


class GuardTestBase(unittest.TestCase):
    def setUp(self):
        self._env_backup = dict(os.environ)
        self.root_tmp = tempfile.TemporaryDirectory()
        self.home_tmp = tempfile.TemporaryDirectory()
        self.log_tmp = tempfile.TemporaryDirectory()
        self.root = self.root_tmp.name
        self.home = self.home_tmp.name

        (Path(self.root) / "crates" / "cleanflate" / "src").mkdir(parents=True)
        (Path(self.root) / "crates" / "oracle" / "src").mkdir(parents=True)
        (Path(self.root) / "tools" / "cleanroom").mkdir(parents=True)
        (Path(self.root) / "docs").mkdir(parents=True)
        (Path(self.root) / "tools" / "cleanroom" / "allowlist.txt").write_text(
            ALLOWLIST_TEXT
        )
        (Path(self.root) / "crates" / "cleanflate" / "src" / "zlib_notes.rs").write_text(
            "// notes mentioning zlib for provenance\n"
        )
        (Path(self.root) / "docs" / "SPEC.md").write_text("# spec\n")
        (Path(self.root) / "target" / "debug" / "build").mkdir(parents=True)
        (Path(self.root) / "target" / "doc" / "src").mkdir(parents=True)

        (Path(self.home) / ".cargo" / "registry").mkdir(parents=True)
        (Path(self.home) / ".cargo" / "git").mkdir(parents=True)
        (Path(self.home) / "zlib-notes.txt").write_text("outside repo\n")

        os.environ["CLEANROOM_REPO_ROOT"] = self.root
        os.environ["CLEANROOM_LOG"] = str(Path(self.log_tmp.name) / "tool-log.jsonl")
        os.environ["CLEANROOM_LOCKDOWN_FILE"] = str(
            Path(self.root) / "tools" / "cleanroom" / "LOCKDOWN"
        )
        os.environ["HOME"] = self.home
        os.environ.pop("CARGO_HOME", None)
        os.environ.pop("FOUNDRY_STAGE", None)

    def tearDown(self):
        os.environ.clear()
        os.environ.update(self._env_backup)
        self.root_tmp.cleanup()
        self.home_tmp.cleanup()
        self.log_tmp.cleanup()

    def evaluate(self, tool_name, tool_input, cwd=None):
        hook_input = {
            "tool_name": tool_name,
            "tool_input": tool_input,
            "cwd": cwd or self.root,
        }
        return guard.evaluate(hook_input)

    def log_lines(self):
        path = os.environ["CLEANROOM_LOG"]
        if not os.path.exists(path):
            return []
        with open(path, "r", encoding="utf-8") as f:
            return [json.loads(line) for line in f if line.strip()]


class TestR1WebSearch(GuardTestBase):
    def test_r1_websearch_denied(self):
        decision, reason = self.evaluate("WebSearch", {"query": "zlib source"})
        self.assertEqual(decision, "denied")
        self.assertIsNotNone(reason)


class TestR2WebFetch(GuardTestBase):
    def test_r2_webfetch_allowlisted_prelock_allowed(self):
        decision, _ = self.evaluate(
            "WebFetch", {"url": "https://www.rfc-editor.org/rfc/rfc1951.txt"}
        )
        self.assertEqual(decision, "allowed")

    def test_r2_webfetch_unlisted_denied(self):
        decision, reason = self.evaluate("WebFetch", {"url": "https://evil.example/x"})
        self.assertEqual(decision, "denied")
        self.assertIsNotNone(reason)

    def test_r2_webfetch_postlock_denied(self):
        Path(os.environ["CLEANROOM_LOCKDOWN_FILE"]).write_text("locked_at=x\n")
        decision, _ = self.evaluate(
            "WebFetch", {"url": "https://www.rfc-editor.org/rfc/rfc1951.txt"}
        )
        self.assertEqual(decision, "denied")


class TestR3Paths(GuardTestBase):
    def test_r3_read_cargo_registry_denied(self):
        target = str(Path(self.home) / ".cargo" / "registry" / "src" / "foo.rs")
        decision, _ = self.evaluate("Read", {"file_path": target})
        self.assertEqual(decision, "denied")

    def test_r3_read_cargo_git_denied(self):
        target = str(Path(self.home) / ".cargo" / "git" / "checkouts" / "foo" / "bar.rs")
        decision, _ = self.evaluate("Read", {"file_path": target})
        self.assertEqual(decision, "denied")

    def test_r3_read_target_build_denied(self):
        target = str(Path(self.root) / "target" / "debug" / "build" / "foo.rs")
        decision, _ = self.evaluate("Read", {"file_path": target})
        self.assertEqual(decision, "denied")

    def test_r3_read_target_doc_src_denied(self):
        target = str(Path(self.root) / "target" / "doc" / "src" / "foo.rs")
        decision, _ = self.evaluate("Read", {"file_path": target})
        self.assertEqual(decision, "denied")

    def test_r3_read_zlib_path_outside_crates_denied(self):
        target = str(Path(self.root) / "tools" / "zlib-notes.txt")
        os.makedirs(str(Path(self.root) / "tools"), exist_ok=True)
        Path(target).write_text("x")
        decision, _ = self.evaluate("Read", {"file_path": target})
        self.assertEqual(decision, "denied")

    def test_r3_read_crate_zlib_rs_allowed(self):
        target = str(Path(self.root) / "crates" / "cleanflate" / "src" / "zlib_notes.rs")
        decision, _ = self.evaluate("Read", {"file_path": target})
        self.assertEqual(decision, "allowed")

    def test_r3_read_outside_repo_gzip_denied(self):
        target = str(Path(self.home) / "some-gzip-notes.txt")
        Path(target).write_text("x")
        decision, _ = self.evaluate("Read", {"file_path": target})
        self.assertEqual(decision, "denied")

    def test_r3_glob_zlib_pattern_repo_root_denied(self):
        decision, _ = self.evaluate("Glob", {"path": self.root, "pattern": "**/zlib*"})
        self.assertEqual(decision, "denied")

    def test_r3_grep_zlib_regex_in_repo_allowed(self):
        decision, _ = self.evaluate(
            "Grep", {"path": self.root, "pattern": "zlib", "glob": "*.md"}
        )
        self.assertEqual(decision, "allowed")


class TestR4ABash(GuardTestBase):
    def test_r4a_curl_allowlisted_prelock_allowed(self):
        decision, _ = self.evaluate(
            "Bash",
            {"command": "curl -fsSL https://www.rfc-editor.org/rfc/rfc1951.txt -o rfc.txt"},
        )
        self.assertEqual(decision, "allowed")

    def test_r4a_curl_unlisted_denied(self):
        decision, _ = self.evaluate("Bash", {"command": "curl -fsSL https://evil.example/x"})
        self.assertEqual(decision, "denied")

    def test_r4a_curl_postlock_denied(self):
        Path(os.environ["CLEANROOM_LOCKDOWN_FILE"]).write_text("locked_at=x\n")
        decision, _ = self.evaluate(
            "Bash",
            {"command": "curl -fsSL https://www.rfc-editor.org/rfc/rfc1951.txt"},
        )
        self.assertEqual(decision, "denied")

    def test_r4a_wget_denied(self):
        decision, _ = self.evaluate("Bash", {"command": "wget https://evil.example/x"})
        self.assertEqual(decision, "denied")

    def test_r4a_gh_denied(self):
        decision, _ = self.evaluate("Bash", {"command": "gh pr create"})
        self.assertEqual(decision, "denied")

    def test_r4a_git_clone_denied(self):
        decision, _ = self.evaluate(
            "Bash", {"command": "git clone https://github.com/madler/zlib"}
        )
        self.assertEqual(decision, "denied")

    def test_r4a_cargo_vendor_denied(self):
        decision, _ = self.evaluate("Bash", {"command": "cargo vendor vendor/"})
        self.assertEqual(decision, "denied")

    def test_r4a_curl_inside_bash_c_denied(self):
        decision, _ = self.evaluate(
            "Bash", {"command": 'bash -c "curl https://evil.example/x"'}
        )
        self.assertEqual(decision, "denied")


class TestR4BFetchScripts(GuardTestBase):
    def test_r4b_fetch_script_postlock_denied(self):
        Path(os.environ["CLEANROOM_LOCKDOWN_FILE"]).write_text("locked_at=x\n")
        decision, _ = self.evaluate(
            "Bash", {"command": "bash tools/cleanroom/fetch-rfcs.sh"}
        )
        self.assertEqual(decision, "denied")


class TestR4CForbiddenRoots(GuardTestBase):
    def test_r4c_cat_registry_denied(self):
        decision, _ = self.evaluate(
            "Bash", {"command": f"cat {self.home}/.cargo/registry/src/foo/lib.rs"}
        )
        self.assertEqual(decision, "denied")


class TestR4DReadFamily(GuardTestBase):
    def test_r4d_find_home_zlib_denied(self):
        decision, _ = self.evaluate(
            "Bash", {"command": f"find {self.home} -iname '*zlib*'"}
        )
        self.assertEqual(decision, "denied")

    def test_r4d_grep_zlib_in_repo_paths_allowed(self):
        decision, _ = self.evaluate(
            "Bash",
            {"command": f"grep -n zlib {self.root}/crates/cleanflate/src/zlib_notes.rs"},
        )
        self.assertEqual(decision, "allowed")

    def test_r4d_grep_r_repo_root_without_exclude_denied(self):
        decision, _ = self.evaluate("Bash", {"command": f"grep -r foo {self.root}"})
        self.assertEqual(decision, "denied")

    def test_r4d_grep_r_with_exclude_dir_allowed(self):
        decision, _ = self.evaluate(
            "Bash", {"command": f"grep -r foo {self.root} --exclude-dir=target"}
        )
        self.assertEqual(decision, "allowed")

    def test_r4d_unparseable_denied(self):
        decision, reason = self.evaluate("Bash", {"command": "cat 'unterminated"})
        self.assertEqual(decision, "denied")
        self.assertIn("unparseable", reason)


class TestPlainAllowed(GuardTestBase):
    def test_cargo_test_allowed(self):
        decision, _ = self.evaluate("Bash", {"command": "cargo test --workspace"})
        self.assertEqual(decision, "allowed")

    def test_git_status_allowed(self):
        decision, _ = self.evaluate("Bash", {"command": "git status --porcelain"})
        self.assertEqual(decision, "allowed")


class TestLoggingAndFailClosed(GuardTestBase):
    def test_denial_is_logged_once(self):
        env = dict(os.environ)
        hook = {"tool_name": "WebSearch", "tool_input": {"query": "zlib"}, "cwd": self.root}
        result = subprocess.run(
            [sys.executable, str(CLEANROOM_DIR / "guard.py")],
            input=json.dumps(hook),
            capture_output=True,
            text=True,
            env=env,
            timeout=10,
        )
        self.assertEqual(result.returncode, 2)
        self.assertIn("cleanroom guard:", result.stderr)
        lines = self.log_lines()
        denied = [line for line in lines if line["decision"] == "denied"]
        self.assertEqual(len(denied), 1)

    def test_internal_error_fails_closed(self):
        # Direct in-process check that main() fails closed (exit 2, denied
        # entry logged) on an exception raised anywhere inside evaluate().
        def raising_evaluate(_hook_input):
            raise RuntimeError("boom")

        real_evaluate = guard.evaluate
        guard.evaluate = raising_evaluate
        try:
            old_stdin = sys.stdin

            class FakeStdin:
                def read(self_inner):
                    return json.dumps(
                        {"tool_name": "Bash", "tool_input": {"command": "echo hi"}}
                    )

            sys.stdin = FakeStdin()
            code = guard.main()
            self.assertEqual(code, 2)
            lines = self.log_lines()
            denied = [line for line in lines if line["decision"] == "denied"]
            self.assertEqual(len(denied), 1)
            self.assertIn("guard internal error", denied[0]["reason"])
        finally:
            sys.stdin = old_stdin
            guard.evaluate = real_evaluate


class TestRealLockdownFile(unittest.TestCase):
    """P0-10: exercises the real tools/cleanroom/LOCKDOWN file this repo
    ships (not a sandboxed fixture), with CLEANROOM_LOCKDOWN_FILE unset so
    guard.py falls back to its default <root>/tools/cleanroom/LOCKDOWN
    path. Requires the real flag file to exist, which P0-10 commits."""

    REAL_ROOT = str(CLEANROOM_DIR.parent.parent)

    def setUp(self):
        self._env_backup = dict(os.environ)
        self.log_tmp = tempfile.TemporaryDirectory()
        os.environ["CLEANROOM_REPO_ROOT"] = self.REAL_ROOT
        os.environ["CLEANROOM_LOG"] = str(Path(self.log_tmp.name) / "tool-log.jsonl")
        os.environ.pop("CLEANROOM_LOCKDOWN_FILE", None)
        os.environ.pop("FOUNDRY_STAGE", None)

    def tearDown(self):
        os.environ.clear()
        os.environ.update(self._env_backup)
        self.log_tmp.cleanup()

    def test_real_lockdown_file_denies_fetch_script(self):
        real_flag = Path(self.REAL_ROOT) / "tools" / "cleanroom" / "LOCKDOWN"
        self.assertTrue(
            real_flag.is_file(),
            "tools/cleanroom/LOCKDOWN must exist for this test (P0-10)",
        )
        hook_input = {
            "tool_name": "Bash",
            "tool_input": {"command": "bash tools/cleanroom/fetch-corpora.sh canterbury"},
            "cwd": self.REAL_ROOT,
        }
        decision, reason = guard.evaluate(hook_input)
        self.assertEqual(decision, "denied")
        self.assertIn("lockdown", reason)


if __name__ == "__main__":
    unittest.main()
