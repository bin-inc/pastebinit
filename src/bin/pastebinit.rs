use pastebinit::cli::{CliOutcome, HelpDefaults, parse_cli};

fn main() {
    let defaults = HelpDefaults::reference_defaults("bpa.st", "maw");
    match parse_cli(std::env::args_os(), &defaults) {
        Ok(CliOutcome::Help(help)) | Ok(CliOutcome::Version(help)) => print!("{help}"),
        Ok(CliOutcome::Run(_)) => {}
        Err(error) => {
            eprintln!("{}", error.message());
            std::process::exit(error.exit_code());
        }
    }
}
