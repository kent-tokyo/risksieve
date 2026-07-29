"""Shared, fail-fast provenance verification for Tian-Bai/SCoRE checkouts.

Used by both `scripts/oracles/generate_score_sdr.py` and
`scripts/audits/compare_score_reference.py` so every tool in this
repository that depends on a specific SCoRE checkout is governed by the
same checks: the checkout's git HEAD, the `SCoRE/SCoRE.py` blob it
contains, and the imported package's `__version__` must all match this
repository's pinned values, or the tool refuses to run. There is no
override flag -- a mismatch always fails, since a fixture or audit result
whose provenance cannot be trusted is worse than one that fails loudly.
"""

import subprocess
import sys

EXPECTED_COMMIT = "401b7caf6d030825ff67e8f08e44ba15ee8c94af"
EXPECTED_VERSION = "0.1.1"
EXPECTED_SCORE_PY_BLOB_SHA = "aa9d111b92fcf574b77f232039410e8a4c23f3f5"
REPOSITORY = "https://github.com/Tian-Bai/SCoRE"
LICENSE = "MIT"
LICENSE_COPYRIGHT = "Copyright (c) 2026 Tian Bai and Ying Jin"


class ProvenanceError(RuntimeError):
    """A SCoRE checkout's commit, blob SHA, or package version does not
    match this repository's pinned provenance."""


def git_rev_parse(repo_path, rev):
    """Runs `git -C <repo_path> rev-parse <rev>` and returns its stdout,
    stripped. Raises ProvenanceError (not a silent None) if the command
    fails for any reason, including `repo_path` not being a git
    repository at all."""
    result = subprocess.run(
        ["git", "-C", str(repo_path), "rev-parse", rev],
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        raise ProvenanceError(
            f"`git -C {repo_path} rev-parse {rev}` failed (is {repo_path} a "
            f"git checkout of {REPOSITORY}?): {result.stderr.strip()}"
        )
    return result.stdout.strip()


def verify_commit(repo_path, expected_commit):
    """Raises `ProvenanceError` unless `repo_path`'s git HEAD is exactly
    `expected_commit`; otherwise returns the (matching) commit SHA. A
    standalone function (rather than inlined in `verify_score_checkout`)
    so it can be tested against a freshly created temporary git repo
    without needing to reproduce a specific pinned commit hash."""
    commit = git_rev_parse(repo_path, "HEAD")
    if commit != expected_commit:
        raise ProvenanceError(
            f"checkout HEAD {commit!r} does not match the expected commit "
            f"{expected_commit!r} -- refusing to proceed with an "
            "unverified checkout"
        )
    return commit


def verify_blob(repo_path, path_in_repo, expected_blob_sha):
    """Raises `ProvenanceError` unless the git blob SHA of `path_in_repo`
    at `repo_path`'s HEAD is exactly `expected_blob_sha`; otherwise
    returns the (matching) blob SHA."""
    blob_sha = git_rev_parse(repo_path, f"HEAD:{path_in_repo}")
    if blob_sha != expected_blob_sha:
        raise ProvenanceError(
            f"{path_in_repo} blob SHA {blob_sha!r} does not match the "
            f"expected blob SHA {expected_blob_sha!r}"
        )
    return blob_sha


def verify_version(actual_version, expected_version):
    """Raises `ProvenanceError` unless `actual_version == expected_version`;
    otherwise returns it."""
    if actual_version != expected_version:
        raise ProvenanceError(
            f"version {actual_version!r} does not match the expected "
            f"version {expected_version!r}"
        )
    return actual_version


def verify_score_checkout(repo_path):
    """Inserts `repo_path` at the front of `sys.path` and verifies its
    checkout HEAD, its `SCoRE/SCoRE.py` blob SHA, and the imported
    `SCoRE.__version__` against this repository's pinned provenance
    (`EXPECTED_COMMIT`, `EXPECTED_SCORE_PY_BLOB_SHA`, `EXPECTED_VERSION`).

    Raises `ProvenanceError` on the first mismatch found, and never
    silently continues past a warning -- callers must fix the checkout
    (or this module's pinned constants, deliberately, in a reviewed
    change) rather than pass a flag to bypass the check.

    Returns a dict of the verified-equal-to-pinned, actually-measured
    values (`commit_sha`, `score_py_blob_sha`, `package_version`),
    suitable for embedding directly in fixture or report provenance
    metadata.
    """
    repo_path = str(repo_path)
    if repo_path not in sys.path:
        sys.path.insert(0, repo_path)

    commit = verify_commit(repo_path, EXPECTED_COMMIT)
    blob_sha = verify_blob(repo_path, "SCoRE/SCoRE.py", EXPECTED_SCORE_PY_BLOB_SHA)

    import SCoRE

    version = verify_version(getattr(SCoRE, "__version__", None), EXPECTED_VERSION)

    return {
        "commit_sha": commit,
        "score_py_blob_sha": blob_sha,
        "package_version": version,
    }


def fail_with_provenance_error(err):
    """Prints a clean, single-line error (not a raw traceback) and exits
    with status 1. Intended for use in a tool's `if __name__ ==
    "__main__"` block: `try: main() except ProvenanceError as err:
    fail_with_provenance_error(err)`."""
    print(f"error: {err}", file=sys.stderr)
    sys.exit(1)
