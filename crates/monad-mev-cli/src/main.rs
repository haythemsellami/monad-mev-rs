fn main() {
    let mut args = std::env::args();
    let _program = args.next();

    match args.next().as_deref() {
        None | Some("-h" | "--help") => {
            print!("{}", monad_mev_cli::help_text());
        }
        Some("--version") => {
            println!("{}", monad_mev_cli::version_text());
        }
        Some(arg) => {
            eprintln!("unknown argument: {arg}");
            eprintln!("try `monad-mev --help`");
            std::process::exit(monad_mev_cli::USAGE_ERROR);
        }
    }
}
