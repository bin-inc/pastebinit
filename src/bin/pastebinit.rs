use pastebinit::cli::{CliOutcome, parse_cli};
use pastebinit::platform::RuntimeEnvironment;
use pastebinit::preferences::{InlineDefaults, help_defaults, load_preferences};

fn main() {
    let environment = RuntimeEnvironment::from_process();
    let inline = InlineDefaults::from_process();
    let preferences =
        match load_preferences(&pastebinit::platform::xml_preference_paths(&environment)) {
            Ok(preferences) => preferences,
            Err(error) => {
                eprintln!("{}", error.message());
                std::process::exit(error.exit_code());
            }
        };
    let defaults = help_defaults("bpa.st", &inline, &preferences);
    match parse_cli(std::env::args_os(), &defaults) {
        Ok(CliOutcome::Help(help)) | Ok(CliOutcome::Version(help)) => print!("{help}"),
        Ok(CliOutcome::Run(_)) => {}
        Err(error) => {
            eprintln!("{}", error.message());
            std::process::exit(error.exit_code());
        }
    }
}
