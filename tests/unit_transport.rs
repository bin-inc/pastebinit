mod support;

use std::net::TcpListener;

use pastebinit::posting::{EncodedBody, UploadPlan};
use pastebinit::transport::{ReqwestTransport, Transport, extract_paste_url, submit_upload};
use support::http_fixture::{FixtureServer, ResponseSpec};

const RESULT_PAGE_ERROR: &str = "Unable to read or parse the result page, it could be a server timeout or a change server side, try with another pastebin.";

fn plan(
    url: impl Into<String>,
    base_url: impl Into<String>,
    body: EncodedBody,
    pattern: Option<&str>,
    target_url: Option<&str>,
) -> UploadPlan {
    UploadPlan {
        url: url.into(),
        base_url: base_url.into(),
        body,
        content_type: None,
        response_pattern: pattern.map(str::to_owned),
        target_url: target_url.map(str::to_owned),
    }
}

fn header<'a>(request: &'a support::http_fixture::RecordedRequest, name: &str) -> &'a str {
    request
        .headers
        .iter()
        .find(|(candidate, _)| candidate.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.trim())
        .unwrap_or_else(|| panic!("request has {name} header"))
}

#[test]
fn capturing_regex_uses_python_re_split_index_one_semantics() {
    assert_eq!(
        extract_paste_url(
            &plan(
                "https://example.test/submit",
                "https://example.test/",
                EncodedBody::Form(Vec::new()),
                Some(r#""data": "([^"]+)""#),
                Some("https://example.test/show/%s/"),
            ),
            "ignored",
            br#"{"data": "abc"}"#,
        )
        .unwrap(),
        "https://example.test/show/abc/"
    );
}

#[test]
fn direct_pattern_returns_the_entire_utf8_body_after_python_stripping() {
    assert_eq!(
        extract_paste_url(
            &plan(
                "https://example.test/submit",
                "https://example.test/",
                EncodedBody::Form(Vec::new()),
                Some("(.*)"),
                None,
            ),
            "ignored",
            b"\nhttps://paste.test/42\t\x1c\x1f",
        )
        .unwrap(),
        "\nhttps://paste.test/42"
    );
}

#[test]
fn target_page_formats_one_string_argument_like_python() {
    for (target_url, expected) in [
        (
            "https://example.test/show/%s",
            "https://example.test/show/abc",
        ),
        (
            "https://example.test/show/%.2s",
            "https://example.test/show/ab",
        ),
        (
            "https://example.test/show/%20s",
            "https://example.test/show/                 abc",
        ),
        (
            "https://example.test/show/%+s",
            "https://example.test/show/abc",
        ),
        (
            "https://example.test/show/%%/%s",
            "https://example.test/show/%/abc",
        ),
    ] {
        assert_eq!(
            extract_paste_url(
                &plan(
                    "https://example.test/submit",
                    "https://example.test/",
                    EncodedBody::Form(Vec::new()),
                    Some("id=(\\w+)"),
                    Some(target_url),
                ),
                "ignored",
                b"id=abc",
            )
            .unwrap(),
            expected
        );
    }
}

#[test]
fn noncapturing_regex_uses_the_post_match_split_segment() {
    assert_eq!(
        extract_paste_url(
            &plan(
                "https://example.test/submit",
                "https://example.test/",
                EncodedBody::Form(Vec::new()),
                Some("id="),
                None,
            ),
            "ignored",
            b"prefix id=42",
        )
        .unwrap(),
        "https://example.test/42"
    );
}

#[test]
fn invalid_regex_and_absent_optional_capture_use_the_reference_error() {
    for pattern in ["(", "id=(a)?b"] {
        let error = extract_paste_url(
            &plan(
                "https://example.test/submit",
                "https://example.test/",
                EncodedBody::Form(Vec::new()),
                Some(pattern),
                None,
            ),
            "ignored",
            b"id=b",
        )
        .unwrap_err();
        assert_eq!(error.message(), RESULT_PAGE_ERROR);
    }
}

#[test]
fn invalid_percent_directives_use_the_reference_error() {
    for target_url in [
        "https://example.test/show",
        "https://example.test/%%s",
        "https://example.test/%q",
        "https://example.test/%*s",
    ] {
        let error = extract_paste_url(
            &plan(
                "https://example.test/submit",
                "https://example.test/",
                EncodedBody::Form(Vec::new()),
                Some("id=(\\d+)"),
                Some(target_url),
            ),
            "ignored",
            b"id=42",
        )
        .unwrap_err();
        assert_eq!(error.message(), RESULT_PAGE_ERROR);
    }
}

#[test]
fn synchronous_transport_submits_without_an_async_runtime() {
    let server = FixtureServer::spawn(vec![ResponseSpec::text(200, "https://paste.test/42")]);
    let upload_plan = plan(
        format!("{}/submit", server.url()),
        format!("{}/", server.url()),
        EncodedBody::Form(Vec::new()),
        Some("(.*)"),
        None,
    );
    assert_eq!(
        submit_upload(&ReqwestTransport::new().unwrap(), &upload_plan).unwrap(),
        "https://paste.test/42"
    );
    let _ = server.next_request();
}

#[test]
fn fixture_invalid_utf8_and_no_match_use_the_reference_error() {
    let invalid_utf8_server = FixtureServer::spawn(vec![ResponseSpec::text(200, vec![0xff])]);
    let invalid_utf8_plan = plan(
        format!("{}/submit", invalid_utf8_server.url()),
        format!("{}/", invalid_utf8_server.url()),
        EncodedBody::Form(Vec::new()),
        Some("(.*)"),
        None,
    );
    let invalid_utf8 =
        submit_upload(&ReqwestTransport::new().unwrap(), &invalid_utf8_plan).unwrap_err();
    assert_eq!(invalid_utf8.message(), RESULT_PAGE_ERROR);
    let _ = invalid_utf8_server.next_request();

    let no_match_server = FixtureServer::spawn(vec![ResponseSpec::text(200, "missing")]);
    let no_match_plan = plan(
        format!("{}/submit", no_match_server.url()),
        format!("{}/", no_match_server.url()),
        EncodedBody::Form(Vec::new()),
        Some("id=(\\d+)"),
        None,
    );
    let no_match = submit_upload(&ReqwestTransport::new().unwrap(), &no_match_plan).unwrap_err();
    assert_eq!(no_match.message(), RESULT_PAGE_ERROR);
    let _ = no_match_server.next_request();
}

#[test]
fn status_and_connection_failures_preserve_source_compatible_categories() {
    let server = FixtureServer::spawn(vec![ResponseSpec::text(500, "failure")]);
    let status_plan = plan(
        format!("{}/submit", server.url()),
        format!("{}/", server.url()),
        EncodedBody::Form(Vec::new()),
        None,
        None,
    );
    let transport = ReqwestTransport::new().unwrap();
    let status_error = transport
        .post(&status_plan)
        .err()
        .expect("HTTP 500 must fail");
    assert_eq!(status_error.message(), "HTTP Error 500");
    let _ = server.next_request();

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let closed_url = format!("http://{}/submit", listener.local_addr().unwrap());
    drop(listener);
    let closed_plan = plan(
        closed_url,
        "http://example.test/",
        EncodedBody::Form(Vec::new()),
        None,
        None,
    );
    let connection_error = transport
        .post(&closed_plan)
        .err()
        .expect("closed socket must fail");
    assert_eq!(
        connection_error.message(),
        "<urlopen error [Errno 111] Connection refused>"
    );
}

#[test]
fn no_match_remains_a_direct_extraction_error() {
    let no_match = extract_paste_url(
        &plan(
            "https://example.test/submit",
            "https://example.test/",
            EncodedBody::Form(Vec::new()),
            Some("id=(\\d+)"),
            None,
        ),
        "ignored",
        b"missing",
    )
    .unwrap_err();
    assert_eq!(no_match.message(), RESULT_PAGE_ERROR);
}

#[test]
fn form_post_uses_reference_headers_body_and_redirected_final_url() {
    let redirect = ResponseSpec {
        status: 302,
        headers: vec![
            ("Location".into(), "/paste/42".into()),
            ("Content-Length".into(), "0".into()),
            ("Connection".into(), "close".into()),
        ],
        body: Vec::new(),
        delay: std::time::Duration::ZERO,
    };
    let server = FixtureServer::spawn(vec![redirect, ResponseSpec::text(200, "complete")]);
    let plan = plan(
        format!("{}/submit", server.url()),
        format!("{}/", server.url()),
        EncodedBody::Form(b"content=a+b%26c".to_vec()),
        None,
        None,
    );

    assert_eq!(
        submit_upload(&ReqwestTransport::new().unwrap(), &plan).unwrap(),
        format!("{}/paste/42", server.url())
    );

    let post = server.next_request();
    assert_eq!(post.method, "POST");
    assert_eq!(post.path, "/submit");
    assert_eq!(
        header(&post, "Content-Type"),
        "application/x-www-form-urlencoded"
    );
    assert_eq!(header(&post, "User-Agent"), "Pastebinit v1.9.0-rc.1");
    assert_eq!(post.body, b"content=a+b%26c");
    let redirect = server.next_request();
    assert_eq!(redirect.method, "GET");
    assert_eq!(redirect.path, "/paste/42");
}

#[test]
fn json_post_uses_the_exact_reference_content_type_and_raw_body() {
    let server = FixtureServer::spawn(vec![ResponseSpec::text(200, "https://paste.test/42")]);
    let plan = plan(
        format!("{}/submit", server.url()),
        format!("{}/", server.url()),
        EncodedBody::Json(b"{\"content\": \"caf\\u00e9\"}".to_vec()),
        Some("(.*)"),
        None,
    );

    assert_eq!(
        submit_upload(&ReqwestTransport::new().unwrap(), &plan).unwrap(),
        "https://paste.test/42"
    );

    let request = server.next_request();
    assert_eq!(header(&request, "Content-Type"), "text/json");
    assert_eq!(header(&request, "User-Agent"), "Pastebinit v1.9.0-rc.1");
    assert_eq!(request.body, b"{\"content\": \"caf\\u00e9\"}");
}
