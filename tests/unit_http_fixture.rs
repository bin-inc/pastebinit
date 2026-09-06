mod support;

use support::http_fixture::{FixtureServer, ResponseSpec};

#[test]
fn fixture_records_a_form_post_and_returns_its_configured_redirect() {
    let server = FixtureServer::spawn(vec![ResponseSpec::redirect("/paste/42")]);
    let response = support::http_fixture::send_test_request(
        &server.url(),
        "POST",
        "/submit",
        b"content=hello",
    );

    assert_eq!(response, 302);
    let request = server.next_request();
    assert_eq!(request.method, "POST");
    assert_eq!(request.path, "/submit");
    assert_eq!(request.body, b"content=hello");
}
