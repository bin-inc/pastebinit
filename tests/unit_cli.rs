use pastebinit::VERSION;
use pastebinit::cli::{CliOutcome, HelpDefaults, parse_cli, render_help};

mod support;

const REFERENCE_HELP: &str = "usage: pastebinit [OPTION...] [FILE...]\n\nReads on stdin for input or takes a list of files as parameters\n\nGeneral arguments:\n  -b <pastebin>         (default is 'bpa.st')\n  -i FILE [FILE ...]    One or more files to read and paste\n  -l                    List all supported pastebins\n  -E                    Print the content to stdout too\n  -h                    Print this help screen\n  -v                    Print the version number\n  -V                    Print verbose output to stderr\n\nOptional arguments (not supported by all pastebins):\n  -a <author>           (default is 'maw')\n  -t <title of paste>   (default is '')\n  -f <format of paste>  (default is 'text')\n  -P <private level>    (default is '1')\n  -e <expiry of paste>  (default is '')\n  -u <username>         (default is '')\n  -p <password>         (default is '')\n";

fn defaults() -> HelpDefaults {
    HelpDefaults::reference_defaults("bpa.st", "maw")
}

#[test]
fn parses_multiple_i_files_and_optional_posting_fields() {
    let outcome = parse_cli(
        [
            "pastebinit",
            "-i",
            "a",
            "b",
            "-a",
            "Ada",
            "-t",
            "Title",
            "-f",
            "text",
            "-P",
            "0",
            "-e",
            "never",
            "-u",
            "user",
            "-p",
            "pass",
            "-E",
            "-V",
        ],
        &defaults(),
    )
    .unwrap();

    let CliOutcome::Run(options) = outcome else {
        panic!("expected run");
    };
    assert_eq!(options.files, vec!["a", "b"]);
    assert_eq!(options.author.as_deref(), Some("Ada"));
    assert_eq!(options.title.as_deref(), Some("Title"));
    assert_eq!(options.format.as_deref(), Some("text"));
    assert_eq!(options.private.as_deref(), Some("0"));
    assert_eq!(options.expiry.as_deref(), Some("never"));
    assert_eq!(options.username.as_deref(), Some("user"));
    assert_eq!(options.password.as_deref(), Some("pass"));
    assert!(options.echo);
    assert!(options.verbose);
}

#[test]
fn appends_positional_files_after_i_files() {
    let outcome = parse_cli(["pastebinit", "-i", "one", "-E", "two"], &defaults()).unwrap();

    let CliOutcome::Run(options) = outcome else {
        panic!("expected run");
    };
    assert_eq!(options.files, vec!["one", "two"]);
}

#[test]
fn accepts_no_arguments_as_an_empty_run() {
    let outcome = parse_cli(["pastebinit"], &defaults()).unwrap();

    let CliOutcome::Run(options) = outcome else {
        panic!("expected run");
    };
    assert!(options.files.is_empty());
}

#[test]
fn parses_a_positional_file() {
    let outcome = parse_cli(["pastebinit", "document"], &defaults()).unwrap();

    let CliOutcome::Run(options) = outcome else {
        panic!("expected run");
    };
    assert_eq!(options.files, vec!["document"]);
}

#[test]
fn appends_repeated_positional_files() {
    let outcome = parse_cli(["pastebinit", "one", "two"], &defaults()).unwrap();

    let CliOutcome::Run(options) = outcome else {
        panic!("expected run");
    };
    assert_eq!(options.files, vec!["one", "two"]);
}

#[test]
fn keeps_list_action_for_later_site_discovery() {
    let outcome = parse_cli(["pastebinit", "-l"], &defaults()).unwrap();

    let CliOutcome::Run(options) = outcome else {
        panic!("expected run");
    };
    assert!(options.list_pastebins);
}

#[test]
fn produces_reference_help_and_version_actions() {
    assert_eq!(render_help(&defaults()), REFERENCE_HELP);

    let help = parse_cli(["pastebinit", "-h"], &defaults()).unwrap();
    assert!(matches!(help, CliOutcome::Help(text) if text == REFERENCE_HELP));

    let version = parse_cli(["pastebinit", "-v"], &defaults()).unwrap();
    assert!(
        matches!(version, CliOutcome::Version(text) if text == format!("pastebinit v{VERSION}\n"))
    );
}

#[test]
fn rejects_help_and_version_tokens_consumed_by_required_options() {
    for arguments in [
        ["pastebinit", "-i", "-h"].as_slice(),
        ["pastebinit", "-a", "-v"].as_slice(),
        ["pastebinit", "-b", "-h"].as_slice(),
    ] {
        let error = parse_cli(arguments, &defaults()).unwrap_err();
        assert_eq!(error.exit_code(), 2);
    }
}

#[test]
fn treats_help_and_version_after_terminator_as_files() {
    for (arguments, expected_file) in [
        (["pastebinit", "--", "-h"].as_slice(), "-h"),
        (["pastebinit", "--", "-v"].as_slice(), "-v"),
    ] {
        let outcome = parse_cli(arguments, &defaults()).unwrap();
        let CliOutcome::Run(options) = outcome else {
            panic!("expected run");
        };
        assert_eq!(options.files, vec![expected_file]);
    }
}

#[test]
fn binary_routes_help_and_version_to_stdout_after_successful_parsing() {
    let help = support::reference::run_rust("pastebinit", &["-h"], b"", &[]);
    assert_eq!(help.code, Some(0));
    assert_eq!(help.stdout, REFERENCE_HELP.as_bytes());
    assert!(help.stderr.is_empty());

    let version = support::reference::run_rust("pastebinit", &["-v"], b"", &[]);
    assert_eq!(version.code, Some(0));
    assert_eq!(
        version.stdout,
        format!("pastebinit v{VERSION}\n").as_bytes()
    );
    assert!(version.stderr.is_empty());
}

#[test]
fn binary_routes_invalid_help_and_version_contexts_to_stderr() {
    for arguments in [["-i", "-h"], ["-a", "-v"], ["-b", "-h"]] {
        let result = support::reference::run_rust("pastebinit", &arguments, b"", &[]);
        assert_eq!(result.code, Some(2));
        assert!(result.stdout.is_empty());
        assert!(!result.stderr.is_empty());
    }
}

#[test]
fn binary_treats_help_and_version_after_terminator_as_files() {
    for arguments in [["--", "-h"], ["--", "-v"]] {
        let result = support::reference::run_rust("pastebinit", &arguments, b"", &[]);
        assert_eq!(result.code, Some(0));
        assert!(result.stdout.is_empty());
        assert!(result.stderr.is_empty());
    }
}

#[test]
fn rejects_unknown_flags_and_i_without_files_as_usage_errors() {
    let unknown = parse_cli(["pastebinit", "-z"], &defaults()).unwrap_err();
    assert_eq!(unknown.exit_code(), 2);

    let missing_files = parse_cli(["pastebinit", "-i"], &defaults()).unwrap_err();
    assert_eq!(missing_files.exit_code(), 2);
}
