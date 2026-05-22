use std::process::Command;

#[test]
fn monad_mev_help_runs() {
    let output = Command::new(env!("CARGO_BIN_EXE_monad-mev"))
        .arg("--help")
        .output()
        .expect("monad-mev binary should run");

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");

    assert!(stdout.contains("Usage: monad-mev"));
}
