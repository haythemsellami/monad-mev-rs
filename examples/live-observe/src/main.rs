fn main() {
    let outcome = monad_mev_cli::run_cli([
        "inspect",
        "monad-exec-events",
        "--live",
        "--duration",
        "1s",
        "--summary",
    ]);
    print!("{}", outcome.stdout);
    eprint!("{}", outcome.stderr);
    std::process::exit(outcome.exit_code);
}

#[cfg(test)]
mod tests {
    #[test]
    fn live_observe_example_runs_in_observe_only_mode() {
        let outcome = monad_mev_cli::run_cli([
            "inspect",
            "monad-exec-events",
            "--live",
            "--duration",
            "1s",
            "--summary",
        ]);

        assert!(outcome.stdout.contains("observe_only: true"));
    }
}
