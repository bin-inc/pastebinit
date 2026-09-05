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
            b"\nhttps://paste.test/42\t",
        )
        .unwrap(),
        "https://paste.test/42"
    );
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
fn malformed_result_pages_use_the_reference_error() {
    let invalid_utf8 = extract_paste_url(
        &plan(
            "https://example.test/submit",
            "https://example.test/",
            EncodedBody::Form(Vec::new()),
            Some("(.*)"),
            None,
        ),
        "ignored",
        &[0xff],
    )
    .unwrap_err();
    assert_eq!(invalid_utf8.message(), RESULT_PAGE_ERROR);

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

    let invalid_format = extract_paste_url(
        &plan(
            "https://example.test/submit",
            "https://example.test/",
            EncodedBody::Form(Vec::new()),
            Some("id=(\\d+)"),
            Some("https://example.test/%%s"),
        ),
        "ignored",
        b"id=42",
    )
    .unwrap_err();
    assert_eq!(invalid_format.message(), RESULT_PAGE_ERROR);
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

#[test]
fn status_and_connection_failures_are_returned_by_the_transport() {
    let server = FixtureServer::spawn(vec![ResponseSpec::text(500, "failure")]);
    let status_plan = plan(
        format!("{}/submit", server.url()),
        format!("{}/", server.url()),
        EncodedBody::Form(Vec::new()),
        None,
        None,
    );
    let transport = ReqwestTransport::new().unwrap();
    assert!(transport.post(&status_plan).is_err());
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
    assert!(transport.post(&closed_plan).is_err());
}
