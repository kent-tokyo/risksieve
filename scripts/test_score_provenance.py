#!/usr/bin/env python3
"""Tests for scripts/score_provenance.py.

Self-contained: builds throwaway temporary git repositories rather than
depending on a real Tian-Bai/SCoRE checkout, so these run without network
access or the actual pinned commit being available. Run with:

    python3 scripts/test_score_provenance.py

or

    python3 -m unittest scripts.test_score_provenance
"""

import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from score_provenance import (
    ProvenanceError,
    git_rev_parse,
    verify_blob,
    verify_commit,
    verify_score_checkout,
    verify_version,
)

SCRIPTS_DIR = str(Path(__file__).resolve().parent)


def make_temp_git_repo(files):
    """Creates a temporary git repository with the given {relative_path:
    content} files committed, and returns (repo_dir, commit_sha)."""
    repo_dir = tempfile.mkdtemp(prefix="score_provenance_test_")
    run = lambda *args: subprocess.run(  # noqa: E731
        ["git", "-C", repo_dir, *args],
        check=True,
        capture_output=True,
        text=True,
    )
    run("init", "--quiet")
    run("config", "user.email", "test@example.invalid")
    run("config", "user.name", "test")
    for relative_path, content in files.items():
        full_path = Path(repo_dir) / relative_path
        full_path.parent.mkdir(parents=True, exist_ok=True)
        full_path.write_text(content)
        run("add", relative_path)
    run("commit", "--quiet", "-m", "test commit")
    commit_sha = git_rev_parse(repo_dir, "HEAD")
    return repo_dir, commit_sha


class GitRevParseTests(unittest.TestCase):
    def test_fails_on_non_git_directory(self):
        with tempfile.TemporaryDirectory() as non_git_dir:
            with self.assertRaises(ProvenanceError):
                git_rev_parse(non_git_dir, "HEAD")


class VerifyCommitTests(unittest.TestCase):
    def test_matching_commit_passes(self):
        repo_dir, commit_sha = make_temp_git_repo({"a.txt": "hello"})
        self.assertEqual(verify_commit(repo_dir, commit_sha), commit_sha)

    def test_mismatched_commit_fails(self):
        repo_dir, commit_sha = make_temp_git_repo({"a.txt": "hello"})
        with self.assertRaises(ProvenanceError):
            verify_commit(repo_dir, "0" * 40)


class VerifyBlobTests(unittest.TestCase):
    def test_matching_blob_passes(self):
        repo_dir, _ = make_temp_git_repo({"SCoRE/SCoRE.py": "print('hi')\n"})
        blob_sha = git_rev_parse(repo_dir, "HEAD:SCoRE/SCoRE.py")
        self.assertEqual(verify_blob(repo_dir, "SCoRE/SCoRE.py", blob_sha), blob_sha)

    def test_mismatched_blob_fails(self):
        repo_dir, _ = make_temp_git_repo({"SCoRE/SCoRE.py": "print('hi')\n"})
        with self.assertRaises(ProvenanceError):
            verify_blob(repo_dir, "SCoRE/SCoRE.py", "0" * 40)

    def test_blob_content_change_is_detected(self):
        # The same file path, different content, must produce a different
        # blob SHA and therefore fail against the original's SHA -- the
        # scenario this whole module exists to catch (a checkout whose
        # source has drifted from the commit it claims to be).
        repo_dir, _ = make_temp_git_repo({"SCoRE/SCoRE.py": "version_a\n"})
        original_blob_sha = git_rev_parse(repo_dir, "HEAD:SCoRE/SCoRE.py")

        other_repo_dir, _ = make_temp_git_repo({"SCoRE/SCoRE.py": "version_b\n"})
        with self.assertRaises(ProvenanceError):
            verify_blob(other_repo_dir, "SCoRE/SCoRE.py", original_blob_sha)


class VerifyVersionTests(unittest.TestCase):
    def test_matching_version_passes(self):
        self.assertEqual(verify_version("0.1.1", "0.1.1"), "0.1.1")

    def test_mismatched_version_fails(self):
        with self.assertRaises(ProvenanceError):
            verify_version("9.9.9", "0.1.1")

    def test_missing_version_fails(self):
        with self.assertRaises(ProvenanceError):
            verify_version(None, "0.1.1")


def make_valid_score_repo(version="0.1.1"):
    """A temp git repo with a minimal but real SCoRE/SCoRE.SCoRE package
    structure (not committed to git -- git presence is irrelevant to
    module-origin checks, only the on-disk file layout matters)."""
    repo_dir = tempfile.mkdtemp(prefix="score_provenance_test_repo_")
    package_dir = Path(repo_dir) / "SCoRE"
    package_dir.mkdir()
    (package_dir / "__init__.py").write_text(f"__version__ = {version!r}\n")
    (package_dir / "SCoRE.py").write_text("value = 1\n")
    return repo_dir


def run_python_subprocess(code):
    """Runs `code` in a brand-new Python process -- a fresh `sys.modules`
    with nothing of this test suite's own imports cached -- and returns
    (returncode, stdout, stderr). Module-origin verification depends on
    import-cache state, so these tests use full process isolation (per
    invocation) rather than saving and restoring `sys.modules`/`sys.path`
    in-process, which would be fragile against partial/nested imports."""
    proc = subprocess.run(
        [sys.executable, "-c", code],
        capture_output=True,
        text=True,
    )
    return proc.returncode, proc.stdout, proc.stderr


class VerifyModuleOriginTests(unittest.TestCase):
    """Each test runs in its own subprocess (see `run_python_subprocess`),
    so no test here can leave a stale `SCoRE` entry in `sys.modules` for
    another test -- or for the rest of this test file -- to trip over.
    """

    def test_accepts_a_freshly_imported_correct_repo(self):
        repo_dir = make_valid_score_repo()
        code = (
            f"import sys; sys.path.insert(0, {SCRIPTS_DIR!r})\n"
            f"sys.path.insert(0, {repo_dir!r})\n"
            "from score_provenance import verify_module_origin\n"
            f"print('RESULT', verify_module_origin({repo_dir!r}))\n"
        )
        returncode, stdout, stderr = run_python_subprocess(code)
        self.assertEqual(returncode, 0, stderr)
        self.assertIn("SCoRE/SCoRE.py", stdout)

    def test_rejects_a_submodule_that_resolves_elsewhere(self):
        # SCoRE/__init__.py is otherwise genuine, but substitutes a
        # foreign module object for the SCoRE.SCoRE submodule before
        # verify_module_origin ever runs -- a pathological case, but
        # exactly what checking SCoRE.SCoRE's own __file__ (not just the
        # top-level package's) exists to catch.
        repo_dir = tempfile.mkdtemp(prefix="score_provenance_test_repo_")
        package_dir = Path(repo_dir) / "SCoRE"
        package_dir.mkdir()
        (package_dir / "__init__.py").write_text(
            "import sys, types\n"
            "fake = types.ModuleType('SCoRE.SCoRE')\n"
            "fake.__file__ = '/nonexistent/elsewhere/SCoRE.py'\n"
            "sys.modules['SCoRE.SCoRE'] = fake\n"
            "__version__ = '0.1.1'\n"
        )
        (package_dir / "SCoRE.py").write_text("value = 1\n")

        code = (
            f"import sys; sys.path.insert(0, {SCRIPTS_DIR!r})\n"
            f"sys.path.insert(0, {repo_dir!r})\n"
            "from score_provenance import verify_module_origin, ProvenanceError\n"
            "try:\n"
            f"    verify_module_origin({repo_dir!r})\n"
            "    print('UNEXPECTED_SUCCESS')\n"
            "except ProvenanceError as e:\n"
            "    print('PROVENANCE_ERROR', e)\n"
            "    sys.exit(1)\n"
        )
        returncode, stdout, stderr = run_python_subprocess(code)
        self.assertNotEqual(returncode, 0, stdout + stderr)
        self.assertIn("PROVENANCE_ERROR", stdout)

    def test_rejects_second_checkout_once_a_different_one_is_cached(self):
        # Both repos report the identical __version__ ("0.1.1") -- this
        # is exactly the scenario version-checking alone would miss:
        # SCoRE is already imported (from repo_a) by the time repo_b is
        # checked, so a bare `import SCoRE` would silently return the
        # repo_a module instead of ever looking at repo_b.
        repo_a = make_valid_score_repo(version="0.1.1")
        repo_b = make_valid_score_repo(version="0.1.1")
        code = (
            f"import sys; sys.path.insert(0, {SCRIPTS_DIR!r})\n"
            "from score_provenance import verify_module_origin, ProvenanceError\n"
            f"sys.path.insert(0, {repo_a!r})\n"
            f"verify_module_origin({repo_a!r})\n"
            "print('FIRST_CHECKOUT_OK')\n"
            "try:\n"
            f"    verify_module_origin({repo_b!r})\n"
            "    print('UNEXPECTED_SUCCESS')\n"
            "except ProvenanceError as e:\n"
            "    print('PROVENANCE_ERROR', e)\n"
            "    sys.exit(1)\n"
        )
        returncode, stdout, stderr = run_python_subprocess(code)
        self.assertNotEqual(returncode, 0, stdout + stderr)
        self.assertIn("FIRST_CHECKOUT_OK", stdout)
        self.assertIn("PROVENANCE_ERROR", stdout)


class VerifyScoreCheckoutIntegrationTests(unittest.TestCase):
    """Exercises verify_score_checkout end to end against a fabricated
    repo standing in for a real SCoRE checkout (which necessarily has a
    different commit SHA than the real, pinned EXPECTED_COMMIT -- a
    fabricated repo cannot reproduce a specific 40-hex commit hash), so a
    wiring mistake in how the three checks are composed (not just each
    check in isolation) would be caught too. This exercises the same
    commit-mismatch path `VerifyCommitTests` does, but through the public
    entry point real callers use."""

    def test_fails_on_a_checkout_that_is_not_the_pinned_commit(self):
        repo_dir, _ = make_temp_git_repo({"SCoRE/SCoRE.py": "print('hi')\n"})
        with self.assertRaises(ProvenanceError) as ctx:
            verify_score_checkout(repo_dir)
        self.assertIn("does not match", str(ctx.exception))


if __name__ == "__main__":
    unittest.main()
