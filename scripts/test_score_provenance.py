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
