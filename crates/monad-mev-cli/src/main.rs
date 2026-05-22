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
        Some("strategy") => match (args.next().as_deref(), args.next()) {
            (Some("new"), Some(destination)) if args.next().is_none() => {
                if let Err(error) = monad_mev_cli::scaffold_strategy_project(&destination) {
                    eprintln!("strategy scaffold failed: {error}");
                    std::process::exit(1);
                }
                println!("created strategy scaffold at {destination}");
            }
            _ => {
                eprintln!("usage: monad-mev strategy new <destination>");
                std::process::exit(monad_mev_cli::USAGE_ERROR);
            }
        },
        Some(arg) => {
            eprintln!("unknown argument: {arg}");
            eprintln!("try `monad-mev --help`");
            std::process::exit(monad_mev_cli::USAGE_ERROR);
        }
    }
}
