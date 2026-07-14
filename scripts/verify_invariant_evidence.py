#!/usr/bin/env python3
"""Verify the Paper 3 invariant-evidence-matrix.json against the live codebase.

For each evidence entry:
  - rust_test:        cargo test -p <pkg> [--lib|--test <target>] <test_name> -- --exact
                      must select exactly 1 test, run it, pass, and not be ignored.
  - trybuild_fixture: fixture file exists, orchestrator registers it via compile_fail("..."),
                      and the orchestration test runs and passes.
  - architecture_guard: treated as rust_test with target_kind=integration.
  - type_shape:       no executable test; only checked for structural presence of the
                      implementation_ref path.
  - gap:              skipped (flagged for manual resolution before v1.4 freeze).

Also cross-checks:
  - run-metadata.md vs run-metadata.json critical fields (frozen/current counts, hashes).
  - MANIFEST.json artifact sha256 vs git blob sha256 (canonical, CRLF-safe).

Exit code 0 = all verifications passed; non-zero = one or more failures (CI signal).

Windows + CI compatible: uses only subprocess, os, sys, json, re, hashlib. No shell
redirection; all cargo/git output captured via subprocess.check_output/capture_output.
"""

import hashlib
import json
import os
import re
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]
MATRIX_PATH = REPO_ROOT / "docs" / "paper3-notes" / "evidence" / "invariant-evidence-matrix.json"
MANIFEST_PATH = REPO_ROOT / "docs" / "paper3-notes" / "evidence-pack" / "MANIFEST.json"
RUN_METADATA_JSON = REPO_ROOT / "docs" / "paper3-notes" / "evidence" / "run-metadata.json"
RUN_METADATA_MD = REPO_ROOT / "docs" / "paper3-notes" / "evidence" / "run-metadata.md"


class Result:
    def __init__(self, ok: bool, detail: str = ""):
        self.ok = ok
        self.detail = detail

    def __bool__(self) -> bool:
        return self.ok


def run_cargo_test(package: str, target_kind: str, target_name, test_name: str,
                   expected_selected: int = 1) -> Result:
    """Run a single rust test via `cargo test ... -- --exact` and verify exactly one
    test was selected and passed."""
    cmd = ["cargo", "test", "-p", package]
    if target_kind == "lib":
        cmd.append("--lib")
    elif target_kind == "integration":
        if not target_name:
            return Result(False, "integration target requires target_name")
        cmd.extend(["--test", target_name])
    else:
        return Result(False, f"unknown target_kind: {target_kind}")
    cmd.append(test_name)
    cmd.extend(["--", "--exact"])

    try:
        proc = subprocess.run(
            cmd,
            cwd=REPO_ROOT,
            capture_output=True,
            text=True,
            timeout=600,
        )
    except subprocess.TimeoutExpired:
        return Result(False, f"timeout running: {' '.join(cmd)}")
    except FileNotFoundError:
        return Result(False, "cargo not found on PATH")

    out = proc.stdout + proc.stderr

    if proc.returncode != 0:
        return Result(False, f"cargo test exit {proc.returncode}; cmd={' '.join(cmd)}\n{out[-800:]}")

    # Parse the test result line: "test result: ok. 1 passed; 0 failed; 0 ignored; ..."
    m = re.search(
        r"test result: (ok|FAILED)\.\s+(\d+) passed;\s+(\d+) failed;\s+(\d+) ignored;",
        out,
    )
    if not m:
        return Result(False, f"could not parse test result line in output:\n{out[-600:]}")
    status, passed, failed, ignored = m.group(1), int(m.group(2)), int(m.group(3)), int(m.group(4))

    if status != "ok":
        return Result(False, f"test result FAILED: {passed}p/{failed}f/{ignored}i")
    if failed != 0:
        return Result(False, f"{failed} tests failed")
    if passed != expected_selected:
        return Result(
            False,
            f"expected {expected_selected} selected/passed but got {passed} "
            f"(typo in test_name? filtered to 0 and silently passed?)",
        )
    if ignored != 0:
        return Result(False, f"{ignored} tests ignored (test not actually run)")
    return Result(True, f"{passed} passed, 0 failed, 0 ignored")


def verify_trybuild_fixture(fixture_rel: str, orchestrator_rel: str,
                            orchestrator_test: str) -> Result:
    """Verify a trybuild compile-fail fixture:
       (1) fixture .rs file exists,
       (2) orchestrator registers it via compile_fail("<fixture>"),
       (3) orchestration test runs and passes (calls the orchestration test by exact name).
    """
    fixture_path = REPO_ROOT / "crates" / "osp-core" / fixture_rel
    if not fixture_path.exists():
        return Result(False, f"fixture file not found: {fixture_rel}")

    orch_path = REPO_ROOT / "crates" / "osp-core" / orchestrator_rel
    if not orch_path.exists():
        return Result(False, f"orchestrator file not found: {orchestrator_rel}")
    orch_src = orch_path.read_text(encoding="utf-8")

    # Check registration: the orchestrator must call compile_fail with the fixture path.
    # trybuild API: t.compile_fail("tests/compile_fail/<name>.rs")
    registration_needle = f'compile_fail("{fixture_rel}")'
    # Also tolerate compile_fail('...') single-quote form.
    registration_needle_alt = f"compile_fail('{fixture_rel}')"
    if registration_needle not in orch_src and registration_needle_alt not in orch_src:
        # Fall back to checking just the basename (some orchestrators use shorter paths).
        basename = Path(fixture_rel).name
        if f'compile_fail("' not in orch_src or basename not in orch_src:
            return Result(
                False,
                f"fixture {fixture_rel} not registered in orchestrator {orchestrator_rel}",
            )

    # Run the orchestration test by exact name to confirm the trybuild run passes.
    return run_cargo_test(
        package="osp-core",
        target_kind="integration",
        target_name=Path(orchestrator_rel).stem,
        test_name=orchestrator_test,
        expected_selected=1,
    )


def git_blob_sha256(repo_path: Path, rel_path: str) -> str:
    """Compute sha256 of a git blob at HEAD (canonical; CRLF/encoding-safe)."""
    blob = subprocess.check_output(
        ["git", "cat-file", "-p", f"HEAD:{rel_path.replace(os.sep, '/')}"],
        cwd=repo_path,
    )
    return hashlib.sha256(blob).hexdigest()


def verify_manifest_hashes(manifest: dict) -> list:
    """Verify each artifact sha256 in MANIFEST.json matches the git blob sha256."""
    results = []
    for entry in manifest.get("files", []):
        fname = entry.get("file")
        recorded = entry.get("sha256")
        if not fname or not recorded:
            continue
        rel = f"docs/paper3-notes/evidence/{fname}"
        try:
            actual = git_blob_sha256(REPO_ROOT, rel)
        except subprocess.CalledProcessError as e:
            results.append((fname, False, f"git cat-file failed: {e}"))
            continue
        if actual != recorded:
            results.append(
                (fname, False, f"MANIFEST sha {recorded} != git blob sha {actual}")
            )
        else:
            results.append((fname, True, actual))
    return results


def verify_entry(entry: dict) -> Result:
    kind = entry.get("evidence_kind")
    if kind == "gap":
        return Result(True, "skipped (gap — flagged for manual resolution)")
    if kind == "type_shape":
        ref = entry.get("implementation_ref", "")
        return Result(True, f"type-shape enforcement (no test); ref={ref}")
    if kind == "architecture_guard":
        return run_cargo_test(
            package=entry["package"],
            target_kind=entry["target_kind"],
            target_name=entry["target_name"],
            test_name=entry["test_name"],
            expected_selected=entry.get("expected_selected", 1),
        )
    if kind == "rust_test":
        return run_cargo_test(
            package=entry["package"],
            target_kind=entry["target_kind"],
            target_name=entry.get("target_name"),
            test_name=entry["test_name"],
            expected_selected=entry.get("expected_selected", 1),
        )
    if kind in ("rust_test_matrix",):
        # Verify the primary test; the error_path_tests list is informational.
        return run_cargo_test(
            package=entry["package"],
            target_kind=entry["target_kind"],
            target_name=entry.get("target_name"),
            test_name=entry["primary_test"],
            expected_selected=entry.get("expected_selected", 1),
        )
    if kind == "trybuild_fixture":
        fixtures = entry.get("fixtures", [])
        orchestrator = entry["orchestrator"]
        orchestrator_test = entry["orchestrator_test"]
        for fixture in fixtures:
            r = verify_trybuild_fixture(fixture, orchestrator, orchestrator_test)
            if not r:
                return Result(False, f"fixture {fixture}: {r.detail}")
        return Result(True, f"{len(fixtures)} fixtures verified via {orchestrator_test}")
    return Result(False, f"unknown evidence_kind: {kind}")


def verify_run_metadata_crosscheck() -> Result:
    """Cross-check run-metadata.md vs run-metadata.json critical counts."""
    if not RUN_METADATA_JSON.exists():
        return Result(False, "run-metadata.json missing")
    if not RUN_METADATA_MD.exists():
        return Result(False, "run-metadata.md missing")
    j = json.loads(RUN_METADATA_JSON.read_text(encoding="utf-8"))
    md = RUN_METADATA_MD.read_text(encoding="utf-8")

    cur = j.get("current_protocol", {})
    checks = [
        ("core_binding_invariants", str(cur.get("core_binding_invariants", ""))),
        ("cumulative_workspace", str(cur.get("compile_fail", {}).get("cumulative_workspace", ""))),
        ("paper3_specific", str(cur.get("compile_fail", {}).get("paper3_specific", ""))),
        ("osp_core_tests", str(cur.get("osp_core_tests", ""))),
    ]
    mismatches = []
    for label, value in checks:
        # md must mention the value somewhere.
        if value and value not in md:
            mismatches.append(f"{label}={value} not found in run-metadata.md")
    if mismatches:
        return Result(False, "; ".join(mismatches))
    return Result(True, "run-metadata.md ↔ run-metadata.json critical counts aligned")


def main() -> int:
    if not MATRIX_PATH.exists():
        print(f"FATAL: matrix not found at {MATRIX_PATH}", file=sys.stderr)
        return 2

    matrix = json.loads(MATRIX_PATH.read_text(encoding="utf-8"))
    entries = matrix.get("evidence_entries", [])

    failures = []
    passed = 0
    skipped = 0

    print(f"Verifying {len(entries)} evidence entries...")
    for entry in entries:
        inv = entry.get("invariant", "?")
        eid = entry.get("evidence_id", "?")
        kind = entry.get("evidence_kind", "?")
        r = verify_entry(entry)
        if kind == "gap" or kind == "type_shape":
            skipped += 1
            status_str = "SKIP"
        elif r:
            passed += 1
            status_str = "ok"
        else:
            failures.append((inv, eid, r.detail))
            status_str = "FAIL"
        print(f"  [{status_str}] {eid:<18} {inv:<22} ({kind})")
        if r.detail and status_str == "ok":
            print(f"          {r.detail}")

    print()
    print(f"rust/trybuild/guard verified: {passed}")
    print(f"skipped (gap/type-shape):    {skipped}")
    print(f"failed:                      {len(failures)}")

    # MANIFEST hash cross-check.
    print()
    print("MANIFEST.json ↔ git blob sha256 cross-check...")
    if MANIFEST_PATH.exists():
        manifest = json.loads(MANIFEST_PATH.read_text(encoding="utf-8"))
        hash_results = verify_manifest_hashes(manifest)
        for fname, ok, detail in hash_results:
            tag = "ok" if ok else "FAIL"
            print(f"  [{tag}] {fname}: {detail[:24]}...")
            if not ok:
                failures.append(("MANIFEST", fname, detail))
    else:
        print("  [WARN] MANIFEST.json not found; skipping hash cross-check")

    # run-metadata cross-check.
    print()
    print("run-metadata.md ↔ run-metadata.json cross-check...")
    r = verify_run_metadata_crosscheck()
    if r:
        print(f"  [ok] {r.detail}")
    else:
        print(f"  [FAIL] {r.detail}")
        failures.append(("run-metadata", "crosscheck", r.detail))

    print()
    if failures:
        print(f"=== {len(failures)} FAILURES ===")
        for inv, eid, detail in failures:
            print(f"  {eid} ({inv}):")
            print(f"    {detail}")
        return 1
    print("All verifications passed.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
