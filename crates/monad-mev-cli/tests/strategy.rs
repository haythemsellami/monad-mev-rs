use std::process::Command;

#[test]
fn strategy_new_command_creates_compiling_project() {
    let destination =
        std::env::temp_dir().join(format!("monad-mev-strategy-command-{}", std::process::id()));
    std::fs::remove_dir_all(&destination).ok();

    let output = Command::new(env!("CARGO_BIN_EXE_monad-mev"))
        .args(["strategy", "new"])
        .arg(&destination)
        .output()
        .expect("monad-mev binary should run");

    assert!(
        output.status.success(),
        "strategy new failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(destination.join("Cargo.toml").is_file());
    assert!(destination.join("src/lib.rs").is_file());

    let output = Command::new("cargo")
        .arg("test")
        .current_dir(&destination)
        .output()
        .expect("generated strategy tests should run");

    std::fs::remove_dir_all(destination).ok();

    assert!(
        output.status.success(),
        "generated strategy tests failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
