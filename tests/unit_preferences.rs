use std::fs;

use pastebinit::cli::{CliOptions, render_help};
use pastebinit::preferences::{
    InlineDefaults, UserPreferences, cli_preferences, effective_option, help_defaults,
    load_preferences, merge_preferences,
};

mod support;

#[test]
fn later_xml_file_and_cli_value_override_earlier_sources() {
    let preferences = load_preferences(&[
        "tests/fixtures/preferences/etc/pastebinit.xml".into(),
        "tests/fixtures/preferences/home/pastebinit.xml".into(),
    ])
    .unwrap();
    assert_eq!(preferences.author.as_deref(), Some("home author"));

    let cli = UserPreferences {
        author: Some("cli author".into()),
        ..UserPreferences::default()
    };
    assert_eq!(
        effective_option(
            "user",
            &cli,
            &preferences,
            &InlineDefaults::for_user("alice")
        ),
        "cli author"
    );
}

#[test]
fn empty_xml_values_are_preserved_not_discarded() {
    let preferences =
        load_preferences(&["tests/fixtures/preferences/empty/pastebinit.xml".into()]).unwrap();
    assert_eq!(preferences.expiry.as_deref(), Some(""));
}

#[test]
fn first_xml_value_is_used_and_self_closing_values_are_empty() {
    let preferences =
        load_preferences(&["tests/fixtures/preferences/first-value/pastebinit.xml".into()])
            .unwrap();

    assert_eq!(preferences.author.as_deref(), Some("first author"));
    assert_eq!(preferences.expiry.as_deref(), Some(""));
}

#[test]
fn malformed_xml_returns_the_reference_configuration_error() {
    let error = load_preferences(&["tests/fixtures/preferences/malformed/pastebinit.xml".into()])
        .unwrap_err();

    assert!(
        error
            .message()
            .contains("Error parsing configuration file!")
    );
}

#[test]
fn xml_document_without_a_root_element_returns_the_reference_configuration_error() {
    let error =
        load_preferences(&["tests/fixtures/preferences/empty-document/pastebinit.xml".into()])
            .unwrap_err();

    assert!(
        error
            .message()
            .contains("Error parsing configuration file!")
    );
}

#[test]
fn xml_document_with_multiple_roots_returns_the_reference_configuration_error() {
    let error =
        load_preferences(&["tests/fixtures/preferences/multiple-roots/pastebinit.xml".into()])
            .unwrap_err();

    assert!(
        error
            .message()
            .contains("Error parsing configuration file!")
    );
}

#[test]
fn xml_document_with_trailing_cdata_returns_the_reference_configuration_error() {
    let error =
        load_preferences(&["tests/fixtures/preferences/trailing-cdata/pastebinit.xml".into()])
            .unwrap_err();

    assert!(
        error
            .message()
            .contains("Error parsing configuration file!")
    );
}

#[test]
fn self_closing_pastebinit_root_produces_default_preferences() {
    let preferences =
        load_preferences(&["tests/fixtures/preferences/self-closing-root/pastebinit.xml".into()])
            .unwrap();

    assert_eq!(preferences, UserPreferences::default());
}

#[test]
fn nested_first_supported_field_preserves_its_empty_direct_text_value() {
    let preferences =
        load_preferences(&["tests/fixtures/preferences/nested-first-value/pastebinit.xml".into()])
            .unwrap();

    assert_eq!(preferences.author.as_deref(), Some(""));
}

#[test]
fn nested_same_name_field_preserves_enclosing_direct_text() {
    let preferences =
        load_preferences(&["tests/fixtures/preferences/nested-same-name/pastebinit.xml".into()])
            .unwrap();

    assert_eq!(preferences.author.as_deref(), Some("beforeafter"));
}

#[test]
fn cli_preferences_and_merge_preserve_cli_precedence_for_all_supported_fields() {
    let cli = CliOptions {
        files: vec![],
        website: Some("cli.test".into()),
        list_pastebins: false,
        echo: false,
        verbose: false,
        author: Some("cli author".into()),
        title: Some("cli title".into()),
        format: Some("cli format".into()),
        private: Some("0".into()),
        expiry: Some("cli expiry".into()),
        username: Some("cli user".into()),
        password: Some("cli password".into()),
    };
    let xml = UserPreferences {
        website: Some("xml.test".into()),
        author: Some("xml author".into()),
        title: Some("xml title".into()),
        format: Some("xml format".into()),
        private: Some("1".into()),
        expiry: Some("xml expiry".into()),
        username: Some("xml user".into()),
        password: Some("xml password".into()),
    };

    assert_eq!(
        merge_preferences(xml, cli_preferences(&cli)),
        UserPreferences {
            website: Some("cli.test".into()),
            author: Some("cli author".into()),
            title: Some("cli title".into()),
            format: Some("cli format".into()),
            private: Some("0".into()),
            expiry: Some("cli expiry".into()),
            username: Some("cli user".into()),
            password: Some("cli password".into()),
        }
    );
}

#[test]
fn help_defaults_use_xml_except_for_inline_expiry() {
    let inline = InlineDefaults::for_user("inline user");
    let xml = UserPreferences {
        website: Some("xml.test".into()),
        author: Some("xml author".into()),
        format: Some("xml format".into()),
        private: Some("0".into()),
        expiry: Some("xml expiry".into()),
        ..UserPreferences::default()
    };

    assert_eq!(
        render_help(&help_defaults("default.test", &inline, &xml)),
        "usage: pastebinit [OPTION...] [FILE...]\n\nReads on stdin for input or takes a list of files as parameters\n\nGeneral arguments:\n  -b <pastebin>         (default is 'xml.test')\n  -i FILE [FILE ...]    One or more files to read and paste\n  -l                    List all supported pastebins\n  -E                    Print the content to stdout too\n  -h                    Print this help screen\n  -v                    Print the version number\n  -V                    Print verbose output to stderr\n\nOptional arguments (not supported by all pastebins):\n  -a <author>           (default is 'xml author')\n  -t <title of paste>   (default is '')\n  -f <format of paste>  (default is 'xml format')\n  -P <private level>    (default is '0')\n  -e <expiry of paste>  (default is '')\n  -u <username>         (default is '')\n  -p <password>         (default is '')\n"
    );
}

#[test]
fn xml_help_defaults_match_the_reference_after_later_file_overrides() {
    let root = tempfile::tempdir().unwrap();
    let system = root.path().join("system");
    let home = root.path().join("home");
    fs::create_dir_all(&system).unwrap();
    fs::create_dir_all(&home).unwrap();
    fs::copy(
        "tests/fixtures/preferences/etc/pastebinit.xml",
        system.join("pastebinit.xml"),
    )
    .unwrap();
    fs::copy(
        "tests/fixtures/preferences/home/pastebinit.xml",
        home.join("pastebinit.xml"),
    )
    .unwrap();
    let env = vec![
        (
            "HOME".into(),
            root.path().join("user").display().to_string(),
        ),
        ("XDG_CONFIG_DIRS".into(), system.display().to_string()),
        ("XDG_CONFIG_HOME".into(), home.display().to_string()),
        ("USER".into(), "reference-user".into()),
        ("LOGNAME".into(), "reference-login".into()),
        ("LC_ALL".into(), "C".into()),
        ("LANG".into(), "C".into()),
        ("TZ".into(), "UTC".into()),
    ];

    let reference = support::reference::run_reference(&["-h"], b"", &env);
    let rust = support::reference::run_rust("pastebinit", &["-h"], b"", &env);
    support::reference::assert_same_process_result(&reference, &rust);
}

#[test]
fn malformed_xml_stops_both_executables_with_the_reference_error() {
    let root = tempfile::tempdir().unwrap();
    let home = root.path().join("home");
    fs::create_dir_all(&home).unwrap();
    fs::copy(
        "tests/fixtures/preferences/malformed/pastebinit.xml",
        home.join("pastebinit.xml"),
    )
    .unwrap();
    let env = vec![
        (
            "HOME".into(),
            root.path().join("user").display().to_string(),
        ),
        ("XDG_CONFIG_DIRS".into(), String::new()),
        ("XDG_CONFIG_HOME".into(), home.display().to_string()),
        ("USER".into(), "reference-user".into()),
        ("LOGNAME".into(), "reference-login".into()),
        ("LC_ALL".into(), "C".into()),
        ("LANG".into(), "C".into()),
        ("TZ".into(), "UTC".into()),
    ];

    let reference = support::reference::run_reference(&[], b"", &env);
    let rust = support::reference::run_rust("pastebinit", &[], b"", &env);
    support::reference::assert_same_process_result(&reference, &rust);
}

#[test]
fn multiple_xml_roots_stop_both_executables_with_the_reference_error() {
    let root = tempfile::tempdir().unwrap();
    let home = root.path().join("home");
    fs::create_dir_all(&home).unwrap();
    fs::copy(
        "tests/fixtures/preferences/multiple-roots/pastebinit.xml",
        home.join("pastebinit.xml"),
    )
    .unwrap();
    let env = vec![
        (
            "HOME".into(),
            root.path().join("user").display().to_string(),
        ),
        ("XDG_CONFIG_DIRS".into(), String::new()),
        ("XDG_CONFIG_HOME".into(), home.display().to_string()),
        ("USER".into(), "reference-user".into()),
        ("LOGNAME".into(), "reference-login".into()),
        ("LC_ALL".into(), "C".into()),
        ("LANG".into(), "C".into()),
        ("TZ".into(), "UTC".into()),
    ];

    let reference = support::reference::run_reference(&[], b"", &env);
    let rust = support::reference::run_rust("pastebinit", &[], b"", &env);
    support::reference::assert_same_process_result(&reference, &rust);
}
