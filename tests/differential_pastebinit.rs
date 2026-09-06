mod support;

use std::fs;
use std::io::Read;
use std::net::TcpListener;
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use support::http_fixture::{FixtureServer, RecordedRequest, ResponseSpec};
use support::reference::{
    ProcessResult, assert_same_process_result, isolated_environment_with_reference_catalog,
    run_reference, run_rust,
};
use tempfile::TempDir;

const FORM_FIXTURE: &str = include_str!("fixtures/differential/form.conf");
const JSON_FIXTURE: &str = include_str!("fixtures/differential/json.conf");
const XML_FIXTURE: &str = include_str!("fixtures/differential/preferences.xml");
const BPA_ST: &str = include_str!("reference/pastebinit-1.8.0/pastebin.d/bpa.st.conf");
const OPENDEV: &str = include_str!("reference/pastebinit-1.8.0/pastebin.d/paste.opendev.org.conf");

struct Environment {
    root: TempDir,
    variables: Vec<(String, String)>,
}

impl Environment {
    fn with_site(definition: String) -> Self {
        let root = tempfile::tempdir().expect("create differential environment");
        let home = root.path().join("home");
        let config = root.path().join("config");
        fs::create_dir_all(config.join("pastebin.d")).expect("create configuration directory");
        fs::create_dir_all(&home).expect("create home directory");
        fs::write(config.join("pastebin.d/local.conf"), definition).expect("write definition");
        Self {
            variables: variables(&home, &config),
            root,
        }
    }

    fn add_config_xml(&self, xml: String) {
        fs::write(self.config().join("pastebinit.xml"), xml).expect("write config XML");
    }

    fn add_home_xml(&self, xml: String) {
        fs::write(self.home().join(".pastebinit.xml"), xml).expect("write home XML");
    }

    fn add_home_definition(&self, name: &str, definition: String) {
        let directory = self.home().join(".pastebin.d");
        fs::create_dir_all(&directory).expect("create home definition directory");
        fs::write(directory.join(name), definition).expect("write home definition");
    }

    fn variables(&self) -> Vec<(String, String)> {
        self.variables.clone()
    }

    fn set_variable(&mut self, name: &str, value: impl Into<String>) {
        self.variables
            .iter_mut()
            .find(|(candidate, _)| candidate == name)
            .expect("known isolated environment variable")
            .1 = value.into();
    }

    fn path(&self, name: &str) -> PathBuf {
        self.root.path().join(name)
    }

    fn home(&self) -> PathBuf {
        self.root.path().join("home")
    }

    fn config(&self) -> PathBuf {
        self.root.path().join("config")
    }
}

fn variables(home: &Path, config: &Path) -> Vec<(String, String)> {
    vec![
        ("HOME".into(), home.to_string_lossy().into_owned()),
        ("XDG_DATA_DIRS".into(), String::new()),
        ("XDG_CONFIG_DIRS".into(), String::new()),
        (
            "XDG_CONFIG_HOME".into(),
            config.to_string_lossy().into_owned(),
        ),
        ("LC_ALL".into(), "C".into()),
        ("LANG".into(), "C".into()),
        ("TZ".into(), "UTC".into()),
        ("USER".into(), "reference-user".into()),
        ("LOGNAME".into(), "reference-login".into()),
    ]
}

fn form_site(authority: &str) -> String {
    FORM_FIXTURE.replace("__AUTHORITY__", authority)
}

fn json_site(authority: &str) -> String {
    JSON_FIXTURE.replace("__AUTHORITY__", authority)
}

fn pair(
    args: &[&str],
    stdin: &[u8],
    environment: &[(String, String)],
) -> (ProcessResult, ProcessResult) {
    let _guard = reference_process_lock()
        .lock()
        .expect("reference process lock is not poisoned");
    let reference = run_reference(args, stdin, environment);
    let rust = run_rust("pastebinit", args, stdin, environment);
    (reference, rust)
}

fn reference_process_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn with_bwrap_requirement<T>(
    requirement: Option<&std::ffi::OsStr>,
    operation: impl FnOnce() -> T,
) -> T {
    let result = {
        let _guard = reference_process_lock()
            .lock()
            .expect("reference process lock is not poisoned");
        let previous = std::env::var_os("PASTEBINIT_REQUIRE_BWRAP");
        match requirement {
            Some(requirement) => unsafe {
                std::env::set_var("PASTEBINIT_REQUIRE_BWRAP", requirement)
            },
            None => unsafe { std::env::remove_var("PASTEBINIT_REQUIRE_BWRAP") },
        }

        let result = catch_unwind(AssertUnwindSafe(operation));
        match previous {
            Some(previous) => unsafe { std::env::set_var("PASTEBINIT_REQUIRE_BWRAP", previous) },
            None => unsafe { std::env::remove_var("PASTEBINIT_REQUIRE_BWRAP") },
        }
        result
    };
    match result {
        Ok(result) => result,
        Err(payload) => resume_unwind(payload),
    }
}

fn assert_pair(args: &[&str], stdin: &[u8], environment: &[(String, String)]) {
    let (reference, rust) = pair(args, stdin, environment);
    assert_same_process_result(&reference, &rust);
}

fn assert_same_request(reference: RecordedRequest, rust: RecordedRequest) {
    assert_eq!(reference.method, rust.method);
    assert_eq!(reference.path, rust.path);
    assert_eq!(reference.body, rust.body);
    let mut reference_headers = normalized_headers(&reference);
    let mut rust_headers = normalized_headers(&rust);
    assert_eq!(
        reference_headers.remove("user-agent"),
        Some("Pastebinit v1.8.0".into())
    );
    assert_eq!(
        rust_headers.remove("user-agent"),
        Some("Pastebinit v1.9.0-rc.1".into())
    );
    assert_eq!(reference_headers, rust_headers);
}

fn assert_same_request_group(server: &FixtureServer, count: usize) {
    let reference: Vec<_> = (0..count).map(|_| server.next_request()).collect();
    let rust: Vec<_> = (0..count).map(|_| server.next_request()).collect();
    for (reference, rust) in reference.into_iter().zip(rust) {
        assert_same_request(reference, rust);
    }
}

fn normalized_headers(request: &RecordedRequest) -> std::collections::BTreeMap<String, String> {
    request
        .headers
        .iter()
        .map(|(name, value)| (name.to_ascii_lowercase(), value.trim().to_owned()))
        .collect()
}

#[test]
fn help_version_and_catalog_match_except_for_declared_versions() {
    let environment = isolated_environment_with_reference_catalog();
    let variables = environment.variables();

    let (reference_help, rust_help) = pair(&["-h"], b"", &variables);
    assert_eq!(reference_help.code, Some(0));
    assert_eq!(reference_help.stderr, rust_help.stderr);
    assert_eq!(
        String::from_utf8(reference_help.stdout)
            .expect("reference help is UTF-8")
            .replace("1.8.0", "1.9.0-rc.1"),
        String::from_utf8(rust_help.stdout).expect("Rust help is UTF-8")
    );

    let (reference_version, rust_version) = pair(&["-v"], b"", &variables);
    assert_eq!(reference_version.code, Some(0));
    assert_eq!(rust_version.code, Some(0));
    assert_eq!(reference_version.stdout, b"pastebinit v1.8.0\n");
    assert_eq!(rust_version.stdout, b"pastebinit v1.9.0-rc.1\n");
    assert!(reference_version.stderr.is_empty());
    assert!(rust_version.stderr.is_empty());

    let (reference_list, rust_list) = pair(&["-l"], b"", &variables);
    let expected = b"Supported pastebins:\n- bpa.st\n- dpaste.com\n- dpaste.org\n- p.defau.lt\n- paste.centos.org\n- paste.debian.net\n- paste.opendev.org\n- paste.opensuse.org\n- paste.ubuntu.com\n- paste.ubuntu.org.cn\n- paste2.org\n- pastebin.com\n- sprunge.us\n";
    assert_eq!(reference_list.stdout, expected);
    assert_eq!(rust_list.stdout, expected);
    assert_eq!(
        reference_list
            .stdout
            .iter()
            .filter(|&&byte| byte == b'\n')
            .count(),
        14
    );
    assert_eq!(
        rust_list
            .stdout
            .iter()
            .filter(|&&byte| byte == b'\n')
            .count(),
        14
    );
    assert_same_process_result(&reference_list, &rust_list);
}

#[test]
fn input_sources_echo_and_posting_options_match_request_bytes() {
    let server = FixtureServer::spawn(vec![
        ResponseSpec::text(200, b"stored".to_vec()),
        ResponseSpec::text(200, b"stored".to_vec()),
        ResponseSpec::text(200, b"stored".to_vec()),
        ResponseSpec::text(200, b"stored".to_vec()),
        ResponseSpec::text(200, b"stored".to_vec()),
        ResponseSpec::text(200, b"stored".to_vec()),
        ResponseSpec::text(200, b"stored".to_vec()),
        ResponseSpec::text(200, b"stored".to_vec()),
        ResponseSpec::text(200, b"stored".to_vec()),
        ResponseSpec::text(200, b"stored".to_vec()),
        ResponseSpec::text(200, b"stored".to_vec()),
        ResponseSpec::text(200, b"stored".to_vec()),
        ResponseSpec::text(200, b"stored".to_vec()),
        ResponseSpec::text(200, b"stored".to_vec()),
    ]);
    let environment = Environment::with_site(form_site(&server.authority()));
    let file_one = environment.path("one.txt");
    let file_two = environment.path("two.txt");
    fs::write(&file_one, "first\n").expect("write first input");
    fs::write(&file_two, "second\n").expect("write second input");
    let one = file_one.to_string_lossy().into_owned();
    let two = file_two.to_string_lossy().into_owned();
    let variables = environment.variables();

    assert_pair(
        &["-b", &server.authority()],
        b"standard input\n\t",
        &variables,
    );
    assert_same_request(server.next_request(), server.next_request());
    assert_pair(&["-b", &server.authority(), &one], b"", &variables);
    assert_same_request(server.next_request(), server.next_request());
    assert_pair(&["-b", &server.authority(), "-i", &one], b"", &variables);
    assert_same_request(server.next_request(), server.next_request());
    assert_pair(&["-b", &server.authority(), &one, &two], b"", &variables);
    assert_same_request_group(&server, 2);
    assert_pair(
        &[
            "-b",
            &server.authority(),
            "-a",
            "Ada",
            "-t",
            "A title",
            "-f",
            "rust",
            "-P",
            "0",
            "-e",
            "tomorrow",
            "-u",
            "name",
            "-p",
            "secret",
        ],
        b"options\n",
        &variables,
    );
    assert_same_request(server.next_request(), server.next_request());
    assert_pair(&["-b", &server.authority(), "-E"], b"echoed\n", &variables);
    assert_same_request(server.next_request(), server.next_request());
}

#[test]
fn xml_cli_and_home_definition_overrides_match() {
    let server = FixtureServer::spawn(vec![
        ResponseSpec::text(200, b"stored".to_vec()),
        ResponseSpec::text(200, b"stored".to_vec()),
        ResponseSpec::text(200, b"stored".to_vec()),
        ResponseSpec::text(200, b"stored".to_vec()),
    ]);
    let environment = Environment::with_site(form_site(&server.authority()));
    environment.add_config_xml(XML_FIXTURE.replace("__AUTHORITY__", &server.authority()));
    environment.add_home_xml(
        "<pastebinit><author>home author</author><format>home-format</format><private>1</private></pastebinit>".into(),
    );
    let variables = environment.variables();
    assert_pair(
        &["-a", "cli author", "-f", "cli-format", "-P", "0"],
        b"xml\n",
        &variables,
    );
    assert_same_request(server.next_request(), server.next_request());

    environment.add_home_definition(
        "override.conf",
        form_site(&server.authority()).replace("post_page = submit", "post_page = home"),
    );
    assert_pair(&["-b", &server.authority()], b"home\n", &variables);
    assert_same_request(server.next_request(), server.next_request());
}

#[test]
fn json_target_redirect_and_verbose_password_follow_the_reference() {
    let server = FixtureServer::spawn(vec![
        ResponseSpec::text(200, b"{\"data\": \"json-result\"}".to_vec()),
        ResponseSpec::text(200, b"{\"data\": \"json-result\"}".to_vec()),
    ]);
    let environment = Environment::with_site(json_site(&server.authority()));
    let variables = environment.variables();
    assert_pair(
        &["-b", &server.authority(), "-V", "-p", "secret"],
        b"json\n",
        &variables,
    );
    assert_same_request(server.next_request(), server.next_request());
}

#[test]
fn command_and_operational_failures_match_the_reference() {
    let server = FixtureServer::spawn(vec![
        ResponseSpec::text(200, vec![0xff]),
        ResponseSpec::text(200, vec![0xff]),
        ResponseSpec::text(200, b"not matching".to_vec()),
        ResponseSpec::text(200, b"not matching".to_vec()),
    ]);
    let environment = Environment::with_site(json_site(&server.authority()));
    let variables = environment.variables();
    for args in [&["--unknown"][..], &["-b"][..], &["-i"][..], &["-z"][..]] {
        let (reference, rust) = pair(args, b"", &variables);
        assert_eq!(reference.code, Some(2));
        assert_eq!(rust.code, Some(2));
        assert_same_process_result(&reference, &rust);
    }

    let missing = environment
        .path("missing.txt")
        .to_string_lossy()
        .into_owned();
    assert_pair(&["-b", &server.authority(), &missing], b"", &variables);
    assert_pair(&["-b", &server.authority()], b"\n\t", &variables);
    assert_pair(&["-b", "unknown.example"], b"body", &variables);
    assert_pair(&["-b", &server.authority()], b"invalid utf8", &variables);
    assert_pair(&["-b", &server.authority()], b"no regexp match", &variables);
}

#[test]
fn malformed_xml_size_limit_and_connection_failure_match() {
    let server = FixtureServer::spawn(vec![ResponseSpec::text(200, b"stored".to_vec())]);
    let oversized = form_site(&server.authority())
        .replace("post_page = submit", "post_page = submit\nsizelimit = 3");
    let environment = Environment::with_site(oversized);
    let variables = environment.variables();
    assert_pair(&["-b", &server.authority()], b"four", &variables);

    environment.add_config_xml("<pastebinit><author>broken</pastebinit>".into());
    assert_pair(&["-h"], b"", &variables);

    let unavailable = Environment::with_site(form_site("127.0.0.1:9"));
    assert_pair(&["-b", "127.0.0.1:9"], b"body", &unavailable.variables());
}

#[test]
fn target_page_warning_is_once_before_all_multi_input_uploads() {
    let server = FixtureServer::spawn(vec![
        ResponseSpec::text(200, b"stored".to_vec()),
        ResponseSpec::text(200, b"stored".to_vec()),
        ResponseSpec::text(200, b"stored".to_vec()),
        ResponseSpec::text(200, b"stored".to_vec()),
    ]);
    let definition = form_site(&server.authority()).replace(
        "post_page = submit",
        "post_page = submit\ntarget_page = result",
    );
    let environment = Environment::with_site(definition);
    let first = environment.path("first.txt");
    let second = environment.path("second.txt");
    fs::write(&first, "first\n").expect("write first document");
    fs::write(&second, "second\n").expect("write second document");
    let first = first.to_string_lossy().into_owned();
    let second = second.to_string_lossy().into_owned();
    let variables = environment.variables();

    assert_pair(
        &["-b", &server.authority(), "-E", "-V", &first, &second],
        b"",
        &variables,
    );
    assert_same_request_group(&server, 2);
}

#[test]
fn reset_loopback_connection_is_not_reported_as_refused() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind reset listener");
    let authority = listener
        .local_addr()
        .expect("read reset listener address")
        .to_string();
    std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept reset connection");
        let mut bytes = [0; 1024];
        let _ = stream.read(&mut bytes);
    });
    let environment = Environment::with_site(form_site(&authority));
    let result = run_rust(
        "pastebinit",
        &["-b", &authority],
        b"body",
        &environment.variables(),
    );
    assert_eq!(result.code, Some(1));
    assert!(String::from_utf8_lossy(&result.stderr).starts_with("Failed to contact the server: "));
    assert!(!String::from_utf8_lossy(&result.stderr).contains("Errno 111"));
}

#[test]
fn executable_adjacent_definition_overrides_xdg_and_home_definitions() {
    let server = FixtureServer::spawn(vec![
        ResponseSpec::text(200, b"stored".to_vec()),
        ResponseSpec::text(200, b"stored".to_vec()),
    ]);
    let environment = Environment::with_site(marked_form_site(&server.authority(), "xdg"));
    environment.add_home_definition("home.conf", marked_form_site(&server.authority(), "home"));
    let reference_directory = environment.path("reference-bin");
    let rust_directory = environment.path("rust-bin");
    let reference = support::reference::prepared_reference_executable(&reference_directory);
    let rust = support::reference::prepared_rust_executable("pastebinit", &rust_directory);
    for directory in [&reference_directory, &rust_directory] {
        fs::create_dir_all(directory.join("pastebin.d")).expect("create executable catalog");
        fs::write(
            directory.join("pastebin.d/adjacent.conf"),
            marked_form_site(&server.authority(), "adjacent"),
        )
        .expect("write executable definition");
    }
    let variables = environment.variables();
    let reference_result = support::reference::run_command(
        &reference,
        &["-b", &server.authority()],
        b"body",
        &variables,
    );
    let rust_result =
        support::reference::run_command(&rust, &["-b", &server.authority()], b"body", &variables);
    assert_same_process_result(&reference_result, &rust_result);
    let reference_request = server.next_request();
    let rust_request = server.next_request();
    assert!(reference_request.body.ends_with(b"marker=adjacent"));
    assert_same_request(reference_request, rust_request);
}

fn marked_form_site(authority: &str, marker: &str) -> String {
    form_site(authority).replace(
        "password = password\n\n[defaults]",
        &format!("password = password\nmarker = marker\n\n[defaults]\nmarker = {marker}"),
    )
}

fn copied_site(definition: &str, basename: &str) -> String {
    definition
        .replace("basename = bpa.st", &format!("basename = {basename}"))
        .replace(
            "basename = paste.opendev.org",
            &format!("basename = {basename}"),
        )
        .replace("https = True", "https =")
}

#[test]
fn xdg_config_and_home_definition_winners_match() {
    let server = FixtureServer::spawn(vec![
        ResponseSpec::text(200, b"stored".to_vec()),
        ResponseSpec::text(200, b"stored".to_vec()),
        ResponseSpec::text(200, b"stored".to_vec()),
        ResponseSpec::text(200, b"stored".to_vec()),
    ]);
    let mut environment = Environment::with_site(marked_form_site(&server.authority(), "home"));
    let system = environment.path("system");
    let later = environment.path("later");
    for (directory, marker) in [(&system, "system"), (&later, "later")] {
        fs::create_dir_all(directory.join("pastebin.d"))
            .expect("create XDG configuration directory");
        fs::write(
            directory.join("pastebin.d/site.conf"),
            marked_form_site(&server.authority(), marker),
        )
        .expect("write XDG definition");
    }
    let empty = environment.path("empty-config");
    fs::create_dir_all(&empty).expect("create empty config home");
    environment.set_variable(
        "XDG_CONFIG_DIRS",
        format!("{}:{}", system.display(), later.display()),
    );
    environment.set_variable("XDG_CONFIG_HOME", empty.display().to_string());
    let variables = environment.variables();
    assert_pair(&["-b", &server.authority()], b"system", &variables);
    let reference = server.next_request();
    let rust = server.next_request();
    assert!(reference.body.ends_with(b"marker=system"));
    assert_same_request(reference, rust);

    environment.set_variable(
        "XDG_CONFIG_HOME",
        environment.config().display().to_string(),
    );
    let variables = environment.variables();
    assert_pair(&["-b", &server.authority()], b"home", &variables);
    let reference = server.next_request();
    let rust = server.next_request();
    assert!(reference.body.ends_with(b"marker=home"));
    assert_same_request(reference, rust);
}

#[test]
fn copied_sites_and_response_modes_match_over_local_http() {
    let server = FixtureServer::spawn(vec![
        ResponseSpec::text(200, b"http://paste.local/bpa".to_vec()),
        ResponseSpec::text(200, b"http://paste.local/bpa".to_vec()),
        ResponseSpec::text(200, b"{\"data\": \"open-dev\"}".to_vec()),
        ResponseSpec::text(200, b"{\"data\": \"open-dev\"}".to_vec()),
        ResponseSpec::redirect("/redirected"),
        ResponseSpec::text(200, b"done".to_vec()),
        ResponseSpec::redirect("/redirected"),
        ResponseSpec::text(200, b"done".to_vec()),
        ResponseSpec::text(200, b"http://paste.local/direct\n".to_vec()),
        ResponseSpec::text(200, b"http://paste.local/direct\n".to_vec()),
        ResponseSpec::text(200, b"prefixid=42".to_vec()),
        ResponseSpec::text(200, b"prefixid=42".to_vec()),
    ]);
    let bpa = Environment::with_site(copied_site(BPA_ST, &server.authority()));
    assert_pair(&["-b", &server.authority()], b"bpa", &bpa.variables());
    assert_same_request(server.next_request(), server.next_request());

    let opendev = Environment::with_site(copied_site(OPENDEV, &server.authority()));
    assert_pair(
        &["-b", &server.authority()],
        b"opendev",
        &opendev.variables(),
    );
    assert_same_request(server.next_request(), server.next_request());

    let redirect = Environment::with_site(form_site(&server.authority()));
    assert_pair(
        &["-b", &server.authority()],
        b"redirect",
        &redirect.variables(),
    );
    let reference_post = server.next_request();
    let reference_redirect = server.next_request();
    let rust_post = server.next_request();
    let rust_redirect = server.next_request();
    assert_same_request(reference_post, rust_post);
    assert_same_request(reference_redirect, rust_redirect);

    let direct = Environment::with_site(form_site(&server.authority()).replace(
        "post_page = submit",
        "post_page = submit\npaste_regexp = (.*)",
    ));
    assert_pair(&["-b", &server.authority()], b"direct", &direct.variables());
    assert_same_request(server.next_request(), server.next_request());

    let no_capture = Environment::with_site(form_site(&server.authority()).replace(
        "post_page = submit",
        "post_page = submit\npaste_regexp = id=",
    ));
    assert_pair(
        &["-b", &server.authority()],
        b"no-capture",
        &no_capture.variables(),
    );
    assert_same_request(server.next_request(), server.next_request());
}

#[test]
fn verbose_password_exposes_the_same_encoded_secret() {
    let server = FixtureServer::spawn(vec![
        ResponseSpec::text(200, b"stored".to_vec()),
        ResponseSpec::text(200, b"stored".to_vec()),
    ]);
    let environment = Environment::with_site(form_site(&server.authority()));
    let (reference, rust) = pair(
        &["-b", &server.authority(), "-V", "-p", "se cret&"],
        b"body",
        &environment.variables(),
    );
    assert_same_process_result(&reference, &rust);
    assert!(
        reference
            .stderr
            .windows(b"password=se+cret%26".len())
            .any(|value| value == b"password=se+cret%26")
    );
    assert!(
        rust.stderr
            .windows(b"password=se+cret%26".len())
            .any(|value| value == b"password=se+cret%26")
    );
    assert_same_request(server.next_request(), server.next_request());
}

#[test]
fn missing_catalog_matches_the_reference() {
    let root = tempfile::tempdir().expect("create empty catalog environment");
    let home = root.path().join("home");
    let config = root.path().join("config");
    fs::create_dir_all(&home).expect("create empty catalog home");
    fs::create_dir_all(&config).expect("create empty catalog config");
    let variables = variables(&home, &config);
    assert_pair(&["-b", "missing.test"], b"body", &variables);
}

#[test]
fn chunked_direct_response_matches_the_reference() {
    let server = FixtureServer::spawn(vec![
        ResponseSpec::chunked(200, b"http://paste.local/chunked\n"),
        ResponseSpec::chunked(200, b"http://paste.local/chunked\n"),
    ]);
    let definition = form_site(&server.authority()).replace(
        "post_page = submit",
        "post_page = submit\npaste_regexp = (.*)",
    );
    let environment = Environment::with_site(definition);
    assert_pair(
        &["-b", &server.authority()],
        b"chunked",
        &environment.variables(),
    );
    assert_same_request(server.next_request(), server.next_request());
}

#[test]
fn invalid_adjacent_configuration_is_hermetic_and_matches() {
    let root = tempfile::tempdir().expect("create malformed configuration root");
    let home = root.path().join("home");
    let config = root.path().join("config");
    fs::create_dir_all(&home).expect("create empty home");
    fs::create_dir_all(&config).expect("create empty XDG config");
    assert!(
        fs::read_dir(&home)
            .expect("read empty home")
            .next()
            .is_none()
    );
    assert!(
        fs::read_dir(&config)
            .expect("read empty XDG config")
            .next()
            .is_none()
    );
    let variables = variables(&home, &config);
    let directory = root.path().join("bin");
    let reference = support::reference::prepared_reference_executable(&directory);
    fs::create_dir_all(directory.join("pastebin.d")).expect("create adjacent configuration");
    fs::write(
        directory.join("pastebin.d/broken.conf"),
        "[other]\nkey = value\n",
    )
    .expect("write invalid adjacent configuration");
    let reference_result = support::reference::run_command(&reference, &["-l"], b"", &variables);
    let rust = support::reference::prepared_rust_executable("pastebinit", &directory);
    let rust_result = support::reference::run_command(&rust, &["-l"], b"", &variables);
    let expected_list = b"Supported pastebins:\n- bpa.st\n- dpaste.com\n- dpaste.org\n- p.defau.lt\n- paste.centos.org\n- paste.debian.net\n- paste.opendev.org\n- paste.opensuse.org\n- paste.ubuntu.com\n- paste.ubuntu.org.cn\n- paste2.org\n- pastebin.com\n- sprunge.us\n";
    assert_eq!(reference_result.stdout, expected_list);
    assert_eq!(rust_result.stdout, expected_list);
    let expected_error = format!(
        "{}: no section [pastebin]\n",
        directory.join("pastebin.d/broken.conf").display()
    );
    assert_eq!(reference_result.stderr, expected_error.as_bytes());
    assert_eq!(rust_result.stderr, expected_error.as_bytes());
    assert_same_process_result(&reference_result, &rust_result);
}

#[test]
fn empty_catalog_list_matches_in_bwrap_namespace() {
    let root = tempfile::tempdir().expect("create empty catalog namespace root");
    let home = root.path().join("home");
    let config = root.path().join("config");
    let data = root.path().join("data");
    let xdg = root.path().join("xdg");
    for directory in [&home, &config, &data, &xdg] {
        fs::create_dir_all(directory).expect("create empty controlled directory");
        assert!(
            fs::read_dir(directory)
                .expect("read controlled directory")
                .next()
                .is_none()
        );
    }
    let reference_directory = root.path().join("reference-bin");
    let rust_directory = root.path().join("rust-bin");
    let reference = support::reference::prepared_reference_executable(&reference_directory);
    let rust = support::reference::prepared_rust_executable("pastebinit", &rust_directory);
    let environment = variables(&home, &config);

    let Some(reference_result) = support::reference::run_command_in_empty_catalog_namespace(
        &reference,
        &["-l"],
        b"",
        &environment,
    ) else {
        return;
    };
    let Some(rust_result) = support::reference::run_command_in_empty_catalog_namespace(
        &rust,
        &["-l"],
        b"",
        &environment,
    ) else {
        return;
    };
    assert_eq!(reference_result.code, Some(0));
    assert_eq!(rust_result.code, Some(0));
    assert_eq!(reference_result.stdout, b"Supported pastebins:\n");
    assert_eq!(rust_result.stdout, b"Supported pastebins:\n");
    assert!(reference_result.stderr.is_empty());
    assert!(rust_result.stderr.is_empty());
    assert_same_process_result(&reference_result, &rust_result);
}

#[test]
fn unavailable_bwrap_skips_normally_and_fails_when_required() {
    let executable = std::env::current_exe().expect("locate differential test executable");
    let missing = std::path::Path::new("/definitely-not-a-bwrap-executable");
    let previous = std::env::var_os("PASTEBINIT_REQUIRE_BWRAP");
    assert!(with_bwrap_requirement(None, || {
        support::reference::run_command_in_empty_catalog_namespace_with_bwrap(
            missing,
            &executable,
            &[],
            b"",
            &[],
        )
        .is_none()
    }));

    let strict = std::panic::catch_unwind(|| {
        with_bwrap_requirement(Some(std::ffi::OsStr::new("1")), || {
            support::reference::run_command_in_empty_catalog_namespace_with_bwrap(
                missing,
                &executable,
                &[],
                b"",
                &[],
            )
        })
    });
    assert!(strict.is_err());
    assert_eq!(std::env::var_os("PASTEBINIT_REQUIRE_BWRAP"), previous);
}

#[test]
fn bwrap_probe_recognizes_non_privileged_user_namespace_denial() {
    let root = tempfile::tempdir().expect("create fake bwrap root");
    let bwrap = root.path().join("bwrap");
    write_executable(
        &bwrap,
        "#!/bin/sh\necho 'bwrap: No permissions to create new namespace: kernel does not allow non-privileged user namespaces' >&2\nexit 1\n",
    );
    let executable = std::env::current_exe().expect("locate differential test executable");

    assert!(with_bwrap_requirement(None, || {
        support::reference::run_command_in_empty_catalog_namespace_with_bwrap(
            &bwrap,
            &executable,
            &[],
            b"",
            &[],
        )
        .is_none()
    }));
}

#[test]
fn child_namespace_denial_diagnostic_remains_a_process_result() {
    let root = tempfile::tempdir().expect("create fake bwrap root");
    let bwrap = root.path().join("bwrap");
    write_executable(
        &bwrap,
        "#!/bin/sh\nshift 15\nif [ \"$1\" = true ]; then\n    exit 0\nfi\nexec \"$@\"\n",
    );
    let child = root.path().join("child");
    write_executable(
        &child,
        "#!/bin/sh\necho 'Creating new namespace failed: Permission denied' >&2\nexit 23\n",
    );

    let result = support::reference::run_command_in_empty_catalog_namespace_with_bwrap(
        &bwrap,
        &child,
        &[],
        b"",
        &[],
    )
    .expect("a child diagnostic must not skip the test");
    assert_eq!(result.code, Some(23));
    assert!(result.stdout.is_empty());
    assert_eq!(
        result.stderr,
        b"Creating new namespace failed: Permission denied\n"
    );
}

#[cfg(unix)]
fn write_executable(path: &std::path::Path, contents: &str) {
    use std::os::unix::fs::PermissionsExt;

    fs::write(path, contents).expect("write executable script");
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("make executable script");
}
