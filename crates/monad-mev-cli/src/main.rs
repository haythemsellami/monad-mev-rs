fn main() {
    let outcome = monad_mev_cli::run_cli(std::env::args().skip(1));

    if !outcome.stdout.is_empty() {
        print!("{}", outcome.stdout);
    }
    if !outcome.stderr.is_empty() {
        eprint!("{}", outcome.stderr);
    }

    std::process::exit(outcome.exit_code);
}
