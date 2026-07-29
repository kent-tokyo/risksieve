"""Shared, fail-fast provenance verification for Tian-Bai/SCoRE checkouts.

Used by both `scripts/oracles/generate_score_sdr.py` and
`scripts/audits/compare_score_reference.py` so every tool in this
repository that depends on a specific SCoRE checkout is governed by the
same checks: the checkout's git HEAD, the `SCoRE/SCoRE.py` blob it
contains, the imported package's `__version__`, and the *actual file
paths* the imported `SCoRE` / `SCoRE.SCoRE` modules resolved to, must all
match this repository's pinned provenance and the requested checkout, or
the tool refuses to run. There is no override flag -- a mismatch always
fails, since a fixture or audit result whose provenance cannot be trusted
is worse than one that fails loudly.

Checking `__version__` alone is not enough: Python caches imported
modules in `sys.modules` keyed by name, so if some *other* SCoRE checkout
or a `pip`-installed copy was imported earlier in the same process, a
later `import SCoRE` silently reuses that cached module regardless of
`sys.path` -- even if it happens to report the same version number. This
module verifies the imported modules' `__file__` actually resolves inside
the checkout being verified, and fails fast (never silently reusing or
evicting a stale cache entry) if it does not.
"""

import subprocess
import sys
from pathlib import Path

EXPECTED_COMMIT = "401b7caf6d030825ff67e8f08e44ba15ee8c94af"
EXPECTED_VERSION = "0.1.1"
EXPECTED_SCORE_PY_BLOB_SHA = "aa9d111b92fcf574b77f232039410e8a4c23f3f5"
REPOSITORY = "https://github.com/Tian-Bai/SCoRE"
LICENSE = "MIT"
LICENSE_COPYRIGHT = "Copyright (c) 2026 Tian Bai and Ying Jin"


class ProvenanceError(RuntimeError):
    """A SCoRE checkout's commit, blob SHA, package version, or imported
    module origin does not match this repository's pinned provenance."""


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


def _expected_module_files(repo_path):
    repo_root = Path(repo_path).resolve()
    return {
        "SCoRE": repo_root / "SCoRE" / "__init__.py",
        "SCoRE.SCoRE": repo_root / "SCoRE" / "SCoRE.py",
    }


def _resolved_file(module):
    file_attr = getattr(module, "__file__", None)
    return Path(file_attr).resolve() if file_attr else None


def verify_module_origin(repo_path):
    """Imports `SCoRE` and `SCoRE.SCoRE` and verifies both resolve to
    files physically inside `repo_path` -- not a stale `sys.modules`
    entry left over from a different checkout or a `pip`-installed copy
    that merely happens to report a matching version number.

    If either module name is already present in `sys.modules` from an
    earlier import elsewhere in this process, and its `__file__` does not
    match the expected path under `repo_path`, this raises
    `ProvenanceError` immediately rather than silently reusing the cached
    module or evicting it from `sys.modules` to force a reimport --
    either of those would hide exactly the mix-up this check exists to
    catch.

    Returns the checkout-relative path of the verified `SCoRE.py`
    (`"SCoRE/SCoRE.py"`), suitable for provenance metadata; never the
    absolute local path.
    """
    expected = _expected_module_files(repo_path)

    for module_name, expected_file in expected.items():
        cached = sys.modules.get(module_name)
        if cached is not None and _resolved_file(cached) != expected_file:
            raise ProvenanceError(
                f"{module_name} is already imported from "
                f"{_resolved_file(cached)!r}, not the expected checkout "
                f"path {expected_file!r} -- refusing to silently reuse a "
                "stale import from a different SCoRE checkout or "
                "installed package; use a fresh Python process for this "
                "checkout"
            )

    import SCoRE
    import SCoRE.SCoRE as score_submodule

    for module_name, module_obj in (("SCoRE", SCoRE), ("SCoRE.SCoRE", score_submodule)):
        actual_file = _resolved_file(module_obj)
        if actual_file != expected[module_name]:
            raise ProvenanceError(
                f"{module_name} resolved to {actual_file!r}, not the "
                f"expected checkout path {expected[module_name]!r}"
            )

    return "SCoRE/SCoRE.py"


def verify_score_checkout(repo_path):
    """Inserts `repo_path` at the front of `sys.path` and verifies its
    checkout HEAD, its `SCoRE/SCoRE.py` blob SHA, that the imported
    `SCoRE` / `SCoRE.SCoRE` modules actually resolve to files inside
    `repo_path` (not a stale cached import from elsewhere), and the
    imported `SCoRE.__version__`, all against this repository's pinned
    provenance (`EXPECTED_COMMIT`, `EXPECTED_SCORE_PY_BLOB_SHA`,
    `EXPECTED_VERSION`).

    Raises `ProvenanceError` on the first mismatch found, and never
    silently continues past a warning -- callers must fix the checkout
    (or this module's pinned constants, deliberately, in a reviewed
    change) rather than pass a flag to bypass the check.

    Returns a dict of the verified-equal-to-pinned, actually-measured
    values (`commit_sha`, `score_py_blob_sha`, `package_version`,
    `imported_score_py_path` -- a checkout-relative path, never absolute),
    suitable for embedding directly in fixture or report provenance
    metadata.
    """
    repo_path = str(repo_path)
    if repo_path not in sys.path:
        sys.path.insert(0, repo_path)

    commit = verify_commit(repo_path, EXPECTED_COMMIT)
    blob_sha = verify_blob(repo_path, "SCoRE/SCoRE.py", EXPECTED_SCORE_PY_BLOB_SHA)
    imported_score_py_path = verify_module_origin(repo_path)

    import SCoRE

    version = verify_version(getattr(SCoRE, "__version__", None), EXPECTED_VERSION)

    return {
        "commit_sha": commit,
        "score_py_blob_sha": blob_sha,
        "package_version": version,
        "imported_score_py_path": imported_score_py_path,
    }


def fail_with_provenance_error(err):
    """Prints a clean, single-line error (not a raw traceback) and exits
    with status 1. Intended for use in a tool's `if __name__ ==
    "__main__"` block: `try: main() except ProvenanceError as err:
    fail_with_provenance_error(err)`."""
    print(f"error: {err}", file=sys.stderr)
    sys.exit(1)


def _self_check_main():
    """`python3 scripts/score_provenance.py --repo <checkout>`: runs
    `verify_score_checkout` standalone and reports success or failure.
    Exists so this module's own checks are subprocess-testable in full
    process isolation (a fresh `sys.modules` per invocation), which
    matters specifically for the module-origin check -- see
    `scripts/test_score_provenance.py`."""
    import argparse

    parser = argparse.ArgumentParser(description=_self_check_main.__doc__)
    parser.add_argument("--repo", required=True)
    args = parser.parse_args()

    result = verify_score_checkout(args.repo)
    print(f"ok: {result}")


if __name__ == "__main__":
    try:
        _self_check_main()
    except ProvenanceError as err:
        fail_with_provenance_error(err)
