#![allow(dead_code)]

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use tempfile::TempDir;

const CLEARED_VARIABLES: [&str; 4] = [
    "HOME",
    "XDG_DATA_DIRS",
    "XDG_CONFIG_DIRS",
    "XDG_CONFIG_HOME",
];

pub struct ProcessResult {
    pub code: Option<i32>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

pub fn run_reference(args: &[&str], stdin: &[u8], env: &[(String, String)]) -> ProcessResult {
    let executable_dir =
        tempfile::tempdir().expect("create isolated reference executable directory");
    let executable = executable_dir.path().join("pastebinit");
    let reference = reference_executable();
    fs::copy(&reference, &executable).expect("copy reference pastebinit executable");
    fs::set_permissions(
        &executable,
        fs::metadata(&reference)
            .expect("read reference pastebinit permissions")
            .permissions(),
    )
    .expect("preserve reference pastebinit permissions");
    run_command(&executable, args, stdin, env)
}

pub fn run_rust(
    binary: &str,
    args: &[&str],
    stdin: &[u8],
    env: &[(String, String)],
) -> ProcessResult {
    run_command(&assert_cmd::cargo::cargo_bin(binary), args, stdin, env)
}

pub fn assert_same_process_result(reference: &ProcessResult, rust: &ProcessResult) {
    if reference.code != rust.code
        || reference.stdout != rust.stdout
        || reference.stderr != rust.stderr
    {
        panic!(
            "process results differ:\nreference exit: {:?}\nrust exit: {:?}\nreference stdout:\n{}\nrust stdout:\n{}\nreference stderr:\n{}\nrust stderr:\n{}",
            reference.code,
            rust.code,
            String::from_utf8_lossy(&reference.stdout),
            String::from_utf8_lossy(&rust.stdout),
            String::from_utf8_lossy(&reference.stderr),
            String::from_utf8_lossy(&rust.stderr),
        );
    }
}

pub struct IsolatedEnvironment {
    root: TempDir,
    variables: Vec<(String, String)>,
}

pub fn isolated_environment_with_site(_basename: &str, definition: &str) -> IsolatedEnvironment {
    let environment = isolated_environment();
    let config_dir = environment.config_dir();
    fs::create_dir_all(&config_dir).expect("create isolated pastebin configuration directory");
    fs::write(config_dir.join("local.conf"), definition)
        .expect("write isolated pastebin definition");
    environment
}

pub fn isolated_environment_with_reference_catalog() -> IsolatedEnvironment {
    let environment = isolated_environment();
    let config_dir = environment.config_dir();
    fs::create_dir_all(&config_dir).expect("create isolated pastebin configuration directory");
    for entry in fs::read_dir(reference_catalog()).expect("read reference pastebin catalog") {
        let entry = entry.expect("read reference pastebin catalog entry");
        let destination = config_dir.join(entry.file_name());
        fs::copy(entry.path(), destination).expect("copy reference pastebin definition");
    }
    environment
}

impl IsolatedEnvironment {
    pub fn variables(&self) -> Vec<(String, String)> {
        self.variables.clone()
    }

    fn config_dir(&self) -> PathBuf {
        PathBuf::from(
            self.variables
                .iter()
                .find(|(name, _)| name == "XDG_CONFIG_HOME")
                .expect("isolated environment has XDG_CONFIG_HOME")
                .1
                .clone(),
        )
        .join("pastebin.d")
    }
}

fn isolated_environment() -> IsolatedEnvironment {
    let root = tempfile::tempdir().expect("create isolated environment root");
    let home = root.path().join("home");
    let config_home = root.path().join("config");
    fs::create_dir_all(&home).expect("create isolated home directory");
    fs::create_dir_all(&config_home).expect("create isolated config directory");
    IsolatedEnvironment {
        variables: vec![
            ("HOME".into(), home.to_string_lossy().into_owned()),
            ("XDG_DATA_DIRS".into(), String::new()),
            ("XDG_CONFIG_DIRS".into(), String::new()),
            (
                "XDG_CONFIG_HOME".into(),
                config_home.to_string_lossy().into_owned(),
            ),
            ("LC_ALL".into(), "C".into()),
            ("LANG".into(), "C".into()),
            ("TZ".into(), "UTC".into()),
            ("USER".into(), "reference-user".into()),
            ("LOGNAME".into(), "reference-login".into()),
        ],
        root,
    }
}

fn run_command(
    executable: &Path,
    args: &[&str],
    stdin: &[u8],
    env: &[(String, String)],
) -> ProcessResult {
    let mut command = Command::new(executable);
    command
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for variable in CLEARED_VARIABLES {
        command.env_remove(variable);
    }
    command.envs(env.iter().map(|(name, value)| (name, value)));

    let mut child = command.spawn().expect("start tested process");
    child
        .stdin
        .take()
        .expect("tested process has stdin")
        .write_all(stdin)
        .expect("write tested process stdin");
    let output = child
        .wait_with_output()
        .expect("read tested process output");
    ProcessResult {
        code: output.status.code(),
        stdout: output.stdout,
        stderr: output.stderr,
    }
}

fn reference_executable() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/reference/pastebinit-1.8.0/pastebinit")
}

fn reference_catalog() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/reference/pastebinit-1.8.0/pastebin.d")
}
