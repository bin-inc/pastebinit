mod support;

#[test]
fn stdin_form_upload_prints_the_redirect_target() {
    let mut redirect = support::http_fixture::ResponseSpec::redirect("/p/42");
    redirect.headers.push(("Connection".into(), "close".into()));
    let server = support::http_fixture::FixtureServer::spawn(vec![
        redirect,
        support::http_fixture::ResponseSpec::text(200, b"stored".to_vec()),
    ]);
    let environment = support::reference::isolated_environment_with_site(
        &server.authority(),
        &format!(
            "[pastebin]\nbasename = {}\npost_page = submit\n\n[format]\ncontent = body\n\n[defaults]\n",
            server.authority()
        ),
    );

    let authority = server.authority();
    let result = support::reference::run_rust(
        "pastebinit",
        &["-b", authority.as_str()],
        b"hello\n",
        &environment.variables(),
    );
    assert_eq!(result.code, Some(0));
    assert_eq!(
        result.stdout,
        format!("{}/p/42\n", server.url()).into_bytes()
    );
}
