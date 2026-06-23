//! Smoke test for `osp-analyze` CLI binary.
//!
//! Core analysis logic (`analyze_repo`) is unit-tested in `pipeline::tests`.
//! Bu test yalnızca binary'nin argüman parse + çalışma + çıktı üretme
//! akışını doğrular — "does it crash on valid input?" sorusu.

use std::fs;
use std::process::Command;
use tempfile::TempDir;

/// Binary path — `CARGO_MANIFEST_DIR` (crates/osp-analyzer) + workspace target.
fn bin() -> String {
    let manifest = env!("CARGO_MANIFEST_DIR");
    format!(
        "{}/../../target/debug/osp-analyze{}",
        manifest,
        std::env::consts::EXE_SUFFIX
    )
}

fn make_fixture() -> TempDir {
    let dir = TempDir::new().unwrap();
    fs::write(
        dir.path().join("main.py"),
        "from utils import helper\n\nclass App:\n    pass\n",
    )
    .unwrap();
    fs::write(dir.path().join("utils.py"), "class Helper:\n    pass\n").unwrap();
    dir
}

#[test]
fn cli_runs_on_valid_repo_and_exits_zero() {
    let dir = make_fixture();
    let output = Command::new(bin())
        .arg(dir.path())
        .output()
        .expect("failed to spawn osp-analyze");

    assert!(
        output.status.success(),
        "osp-analyze should exit 0 on valid repo; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn cli_produces_table_output() {
    let dir = make_fixture();
    let output = Command::new(bin())
        .arg(dir.path())
        .output()
        .expect("failed to spawn");

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Binary bir tablo basar — en az fromsatır olmalı (header + data)
    assert!(!stdout.is_empty(), "should produce output");
}

#[test]
fn cli_handles_multiple_repos() {
    let dir1 = make_fixture();
    let dir2 = TempDir::new().unwrap();
    fs::write(dir2.path().join("a.py"), "class A:\n    pass\n").unwrap();

    let output = Command::new(bin())
        .arg(dir1.path())
        .arg(dir2.path())
        .output()
        .expect("failed to spawn");

    assert!(output.status.success(), "should handle multiple repos");
}

#[test]
fn cli_empty_dir_exits_zero() {
    let dir = TempDir::new().unwrap();
    let output = Command::new(bin()).arg(dir.path()).output().expect("spawn");
    assert!(
        output.status.success(),
        "empty dir should not crash; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
