use std::path::PathBuf;

use pastebinit::config::{load_site_catalog, resolve_site};

#[test]
fn later_definition_replaces_the_complete_site_definition() {
    let mut diagnostics = Vec::new();
    let catalog = load_site_catalog(
        &[
            "tests/fixtures/config/system/pastebin.d".into(),
            "tests/fixtures/config/user/pastebin.d".into(),
        ],
        &mut diagnostics,
    )
    .unwrap();

    let site = catalog.get("example.test").unwrap();
    assert_eq!(site.pastebin("post_page"), Some("user-submit"));
    assert_eq!(site.format.get("content").map(String::as_str), Some("body"));
    assert_eq!(site.format.get("title"), None);
    assert_eq!(site.defaults.get("expiry").map(String::as_str), Some("day"));
    assert_eq!(
        site.source,
        PathBuf::from("tests/fixtures/config/user/pastebin.d/example.conf")
    );
}

#[test]
fn target_page_unescapes_configparser_percent_escape() {
    let mut diagnostics = Vec::new();
    let catalog = load_site_catalog(
        &["tests/fixtures/config/interpolation/pastebin.d".into()],
        &mut diagnostics,
    )
    .unwrap();

    let site = catalog.get("json.test").unwrap();
    assert_eq!(site.pastebin("target_page"), Some("show/%s/"));
    assert_eq!(site.pastebin("post_page"), Some("api/v1/paste"));
}

#[test]
fn catalog_preserves_defaults_interpolation_for_posting_time_variables() {
    let mut diagnostics = Vec::new();
    let catalog = load_site_catalog(
        &["tests/fixtures/config/default-interpolation/pastebin.d".into()],
        &mut diagnostics,
    )
    .unwrap();

    let site = catalog.get("loaded.test").unwrap();
    assert_eq!(
        site.defaults.get("custom").map(String::as_str),
        Some("chosen-%(format)s")
    );
    assert!(diagnostics.is_empty());
}

#[test]
fn missing_interpolation_key_returns_an_error() {
    let mut diagnostics = Vec::new();
    let result = load_site_catalog(
        &["tests/fixtures/config/missing-interpolation/pastebin.d".into()],
        &mut diagnostics,
    );
    let error = match result {
        Err(error) => error,
        Ok(_) => panic!("missing interpolation key was accepted"),
    };

    assert!(
        error
            .message()
            .contains("missing interpolation key 'endpoint'")
    );
    assert!(diagnostics.is_empty());
}

#[test]
fn invalid_definitions_are_skipped_with_reference_diagnostics() {
    let mut diagnostics = Vec::new();
    let catalog = load_site_catalog(
        &["tests/fixtures/config/invalid/pastebin.d".into()],
        &mut diagnostics,
    )
    .unwrap();

    assert!(catalog.names_sorted().is_empty());
    assert_eq!(
        String::from_utf8(diagnostics).unwrap(),
        "tests/fixtures/config/invalid/pastebin.d/no-basename.conf: no 'basename' in [pastebin]\n\
tests/fixtures/config/invalid/pastebin.d/no-pastebin.conf: no section [pastebin]\n"
    );
}

#[test]
fn hidden_and_non_configuration_files_are_ignored() {
    let mut diagnostics = Vec::new();
    let catalog = load_site_catalog(
        &["tests/fixtures/config/ignored/pastebin.d".into()],
        &mut diagnostics,
    )
    .unwrap();

    assert!(catalog.names_sorted().is_empty());
    assert!(diagnostics.is_empty());
}

#[test]
fn resolve_site_normalizes_input_and_treats_nonempty_https_as_enabled() {
    let mut diagnostics = Vec::new();
    let catalog = load_site_catalog(
        &[
            "tests/fixtures/config/interpolation/pastebin.d".into(),
            "tests/fixtures/config/http/pastebin.d".into(),
        ],
        &mut diagnostics,
    )
    .unwrap();

    let (https_site, https_url) = resolve_site(&catalog, "https://json.test///").unwrap();
    assert_eq!(https_site.basename, "json.test");
    assert_eq!(https_url, "https://json.test/");

    let (http_site, http_url) = resolve_site(&catalog, "/plain.test/").unwrap();
    assert_eq!(http_site.basename, "plain.test");
    assert_eq!(http_url, "http://plain.test/");
}

#[test]
fn copied_reference_catalog_has_the_shipped_site_names() {
    let mut diagnostics = Vec::new();
    let catalog = load_site_catalog(
        &["tests/reference/pastebinit-1.8.0/pastebin.d".into()],
        &mut diagnostics,
    )
    .unwrap();

    let names = catalog.names_sorted();
    assert_eq!(names.len(), 13);
    for name in [
        "bpa.st",
        "paste.debian.net",
        "paste.opendev.org",
        "sprunge.us",
    ] {
        assert!(names.contains(&name));
    }
    assert!(diagnostics.is_empty());
}
