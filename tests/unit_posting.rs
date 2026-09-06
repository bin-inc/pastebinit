use std::path::PathBuf;

use indexmap::IndexMap;
use pastebinit::config::{SiteDefinition, load_site_catalog};
use pastebinit::posting::{EncodedBody, build_upload_plan};
use pastebinit::preferences::{InlineDefaults, UserPreferences};

fn site(
    pastebin: impl IntoIterator<Item = (&'static str, &'static str)>,
    format: impl IntoIterator<Item = (&'static str, &'static str)>,
    defaults: impl IntoIterator<Item = (&'static str, &'static str)>,
) -> SiteDefinition {
    let pastebin: IndexMap<String, String> = pastebin
        .into_iter()
        .map(|(key, value)| (key.into(), value.into()))
        .collect();
    let basename = pastebin.get("basename").unwrap().clone();
    SiteDefinition {
        basename,
        pastebin,
        format: format
            .into_iter()
            .map(|(key, value)| (key.into(), value.into()))
            .collect(),
        defaults: defaults
            .into_iter()
            .map(|(key, value)| (key.into(), value.into()))
            .collect(),
        source: PathBuf::from("fixture.conf"),
    }
}

fn reference_site(basename: &str) -> SiteDefinition {
    let mut diagnostics = Vec::new();
    let catalog = load_site_catalog(
        &["tests/reference/pastebinit-1.8.0/pastebin.d".into()],
        &mut diagnostics,
    )
    .unwrap();
    assert!(diagnostics.is_empty());
    let definition = catalog.get(basename).unwrap();
    SiteDefinition {
        basename: definition.basename.clone(),
        pastebin: definition.pastebin.clone(),
        format: definition.format.clone(),
        defaults: definition.defaults.clone(),
        source: definition.source.clone(),
    }
}

#[test]
fn form_payload_preserves_format_order_and_python_plus_encoding() {
    let definition = site(
        [
            ("basename", "example.test"),
            ("https", "True"),
            ("post_page", "submit"),
        ],
        [("user", "author"), ("content", "content")],
        [],
    );
    let cli = UserPreferences {
        author: Some("Ada".into()),
        ..UserPreferences::default()
    };

    let plan = build_upload_plan(
        &definition,
        &cli,
        &UserPreferences::default(),
        &InlineDefaults::for_user("alice"),
        "a b&c",
    )
    .unwrap();

    assert_eq!(plan.url, "https://example.test/submit");
    assert_eq!(plan.base_url, "https://example.test/");
    assert_eq!(
        plan.body,
        EncodedBody::Form(b"author=Ada&content=a+b%26c".to_vec())
    );
    assert_eq!(plan.content_type, None);
}

#[test]
fn json_payload_matches_python_json_dumps_spacing_and_ascii_escaping() {
    let definition = site(
        [
            ("basename", "json.test"),
            ("https", "True"),
            ("post_format", "json"),
        ],
        [("content", "content")],
        [],
    );

    let plan = build_upload_plan(
        &definition,
        &UserPreferences::default(),
        &UserPreferences::default(),
        &InlineDefaults::for_user("alice"),
        "caf\u{00e9}",
    )
    .unwrap();

    assert_eq!(
        plan.body,
        EncodedBody::Json(b"{\"content\": \"caf\\u00e9\"}".to_vec())
    );
    assert_eq!(plan.content_type.as_deref(), Some("text/json"));
}

#[test]
fn bpa_st_json_uses_reference_order_and_site_default() {
    let definition = reference_site("bpa.st");
    let cli = UserPreferences {
        title: Some("A paste".into()),
        format: Some("rust".into()),
        ..UserPreferences::default()
    };

    let plan = build_upload_plan(
        &definition,
        &cli,
        &UserPreferences::default(),
        &InlineDefaults::for_user("alice"),
        "let x = \"\u{00e9}\";",
    )
    .unwrap();

    assert_eq!(plan.url, "https://bpa.st/api/v1/paste");
    assert_eq!(plan.response_pattern.as_deref(), Some("(.*)"));
    assert_eq!(plan.target_url, None);
    assert_eq!(
        plan.body,
        EncodedBody::Json(
            b"{\"name\": \"A paste\", \"content\": \"let x = \\\"\\u00e9\\\";\", \"lexer\": \"rust\", \"expiry\": \"1month\"}".to_vec()
        )
    );
}

#[test]
fn opendev_json_keeps_target_page_for_response_processing() {
    let definition = reference_site("paste.opendev.org");
    let cli = UserPreferences {
        format: Some("rust".into()),
        ..UserPreferences::default()
    };

    let plan = build_upload_plan(
        &definition,
        &cli,
        &UserPreferences::default(),
        &InlineDefaults::for_user("alice"),
        "x",
    )
    .unwrap();

    assert_eq!(
        plan.url,
        "https://paste.opendev.org/json/?method=pastes.newPaste"
    );
    assert_eq!(
        plan.response_pattern.as_deref(),
        Some("\"data\": \"([^\"]+)\"")
    );
    assert_eq!(
        plan.target_url.as_deref(),
        Some("https://paste.opendev.org/show/%s/")
    );
    assert_eq!(
        plan.body,
        EncodedBody::Json(
            b"{\"code\": \"x\", \"language\": \"rust\", \"private\": \"yes\"}".to_vec()
        )
    );
}

#[test]
fn debian_user_length_truncates_unicode_scalars_before_encoding() {
    let definition = reference_site("paste.debian.net");
    let cli = UserPreferences {
        author: Some("abcdefghi\u{00e9}x".into()),
        ..UserPreferences::default()
    };

    let plan = build_upload_plan(
        &definition,
        &cli,
        &UserPreferences::default(),
        &InlineDefaults::for_user("alice"),
        "x",
    )
    .unwrap();

    assert_eq!(
        plan.body,
        EncodedBody::Form(
            b"poster=abcdefghi~&code=x&lang=text&remember=0&expire=604800&private=1".to_vec()
        )
    );
}

#[test]
fn custom_non_ascii_site_default_is_form_encoded_as_utf8() {
    let definition = reference_site("paste.ubuntu.org.cn");

    let plan = build_upload_plan(
        &definition,
        &UserPreferences::default(),
        &UserPreferences::default(),
        &InlineDefaults::for_user("alice"),
        "x",
    )
    .unwrap();

    assert_eq!(
        plan.body,
        EncodedBody::Form(b"poster=alice&code2=x&class=text&paste=%E5%8F%91%E9%80%81".to_vec())
    );
}

#[test]
fn cli_and_xml_values_override_defaults_while_defaults_interpolate_them() {
    let definition = site(
        [
            ("basename", "precedence.test"),
            ("post_page", "/submit"),
            ("post_format", "standard"),
            ("paste_regexp", "result=(.*)"),
            ("target_page", "/show/%s"),
            ("user_length", "6"),
        ],
        [
            ("user", "author"),
            ("title", "title"),
            ("format", "format"),
            ("private", "private"),
            ("custom", "custom"),
            ("content", "content"),
        ],
        [
            ("title", "site title"),
            ("format", "site format"),
            ("private", "site private"),
            ("custom", "chosen-%(format)s"),
        ],
    );
    let cli = UserPreferences {
        title: Some("cli title".into()),
        format: Some("cli format".into()),
        private: Some("cli private".into()),
        ..UserPreferences::default()
    };
    let xml = UserPreferences {
        title: Some("xml title".into()),
        format: Some("xml format".into()),
        private: Some("xml private".into()),
        ..UserPreferences::default()
    };

    let plan = build_upload_plan(
        &definition,
        &cli,
        &xml,
        &InlineDefaults::for_user("alice"),
        "x",
    )
    .unwrap();

    assert_eq!(plan.url, "http://precedence.test/submit");
    assert_eq!(
        plan.target_url.as_deref(),
        Some("http://precedence.test/show/%s")
    );
    assert_eq!(
        plan.body,
        EncodedBody::Form(
            b"author=alice&title=cli+title&format=cli+format&private=cli+private&custom=chosen-cli+format&content=x".to_vec()
        )
    );
}

#[test]
fn catalog_loaded_default_interpolates_the_cli_format_override() {
    let mut diagnostics = Vec::new();
    let catalog = load_site_catalog(
        &["tests/fixtures/config/default-interpolation/pastebin.d".into()],
        &mut diagnostics,
    )
    .unwrap();
    assert!(diagnostics.is_empty());
    let cli = UserPreferences {
        format: Some("cli-format".into()),
        ..UserPreferences::default()
    };

    let plan = build_upload_plan(
        catalog.get("loaded.test").unwrap(),
        &cli,
        &UserPreferences::default(),
        &InlineDefaults::for_user("alice"),
        "x",
    )
    .unwrap();

    assert_eq!(
        plan.body,
        EncodedBody::Form(b"format=cli-format&custom=chosen-cli-format&content=x".to_vec())
    );
}

#[test]
fn catalog_loaded_default_interpolates_the_xml_format_override() {
    let mut diagnostics = Vec::new();
    let catalog = load_site_catalog(
        &["tests/fixtures/config/default-interpolation/pastebin.d".into()],
        &mut diagnostics,
    )
    .unwrap();
    assert!(diagnostics.is_empty());
    let xml = UserPreferences {
        format: Some("xml-format".into()),
        ..UserPreferences::default()
    };

    let plan = build_upload_plan(
        catalog.get("loaded.test").unwrap(),
        &UserPreferences::default(),
        &xml,
        &InlineDefaults::for_user("alice"),
        "x",
    )
    .unwrap();

    assert_eq!(
        plan.body,
        EncodedBody::Form(b"format=xml-format&custom=chosen-xml-format&content=x".to_vec())
    );
}

#[test]
fn size_limit_counts_unicode_scalars_and_rejects_before_planning() {
    let definition = site(
        [("basename", "limit.test"), ("sizelimit", "2")],
        [("content", "content")],
        [],
    );

    let error = build_upload_plan(
        &definition,
        &UserPreferences::default(),
        &UserPreferences::default(),
        &InlineDefaults::for_user("alice"),
        "\u{00e9}\u{00e9}x",
    )
    .unwrap_err();

    assert!(
        error
            .message()
            .contains("exceeds the pastebin's size limit")
    );
}
