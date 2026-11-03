//! Compile-fail checks: a literal that breaks a refinement must not compile.
//!
//! The fixture is built with `cargo build` and the test asserts on the outcome
//! and on the stable part of the message. It does not compare the whole rustc
//! output, because the wording of `E0080` changes between toolchains and a
//! full snapshot would pass on one toolchain only.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const FIXTURE: &str = include_str!("ui/refined_bad_literal.rs");

#[test]
fn refined_literal_is_checked_at_compile_time() {
    let root = env::temp_dir().join("tpt-reficon-ui");
    let fixture = root.join("refined_bad_literal");
    write_fixture(&fixture);

    // A separate target directory keeps the fixture's build away from the
    // outer `cargo test`. It is reused across runs, so only the first run pays
    // for compiling the dependencies.
    let output = Command::new(env!("CARGO"))
        .arg("build")
        .arg("--quiet")
        .current_dir(&fixture)
        .env("CARGO_TARGET_DIR", root.join("target"))
        .output()
        .expect("failed to run cargo on the fixture");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "refined! accepted a literal that violates the refinement:\n{stderr}"
    );
    assert!(
        stderr.contains("error[E0080]"),
        "expected a const-evaluation error, got:\n{stderr}"
    );
    assert!(
        stderr.contains("refinement violated by the literal"),
        "expected the refinement message, got:\n{stderr}"
    );
}

/// Writes a standalone crate that depends on this one by path, with the
/// fixture as its only source file.
fn write_fixture(dir: &Path) {
    let src = dir.join("src");
    fs::create_dir_all(&src).expect("failed to create the fixture directory");

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let manifest = format!(
        "[package]\n\
         name = \"refined_bad_literal\"\n\
         version = \"0.0.0\"\n\
         edition = \"2021\"\n\
         \n\
         [workspace]\n\
         \n\
         [dependencies]\n\
         tpt-reficon = {{ path = {:?} }}\n",
        manifest_dir.display().to_string()
    );
    fs::write(dir.join("Cargo.toml"), manifest).expect("failed to write the fixture manifest");
    fs::write(src.join("main.rs"), FIXTURE).expect("failed to write the fixture source");
}
