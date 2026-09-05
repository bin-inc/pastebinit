# pastebinit 1.9.0 Rust Rewrite Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Release `v1.9.0` as a Debian-installable native Rust replacement for pastebinit 1.8.0, including its installed `pastebinit` and `pbput` commands and differential compatibility tests.

**Architecture:** One Cargo package exposes a reusable `pastebinit` library and two thin binaries. The library owns CLI compatibility, configuration discovery, input processing, posting, response parsing, localization, and the `pbput` codec/workflow; binaries only connect process I/O to those interfaces. A checked-in, unmodified 1.8.0 reference fixture and a local HTTP fixture server make differential tests hermetic and independent of public pastebin services.

**Tech Stack:** Rust stable edition 2024; `clap`, `indexmap`, `quick-xml`, `regex`, `reqwest` blocking client with Rustls, `tempfile`, `signal-hook`, `gettextrs`; GNU `tar`, `lzma`, and `base64` for `pbput`-compatible transforms; `assert_cmd` for tests; Debian `debhelper` and `dh-cargo`; GitHub Actions.

**Spec:** `docs/superpowers/specs/2026-09-05-pastebinit-rust-design.md`

## Global Constraints

- Repository and upstream identity: `https://github.com/bin-inc/pastebinit`.
- Cargo crate, Debian source package, Debian binary package, and primary executable are all named `pastebinit`.
- Ship only `pastebinit` and `pbput`, exactly as Debian pastebinit 1.8.0 installs them; do not leave the legacy shell helper script in the release package.
- Treat `/home/maw/src/bin-inc/origin-sources/pastebinit-1.8.0` as the 1.8.0 behavioral reference and do not modify it.
- Preserve GPL-2.0-or-later licensing, applicable upstream copyright notices, and translation attribution.
- Preserve the 1.8.0 command options, output streams, exit status, configuration syntax, configuration search ordering, shipped pastebin definitions, and 15-second request timeout unless an intentional deviation is documented and tested.
- Support `pastebin.d` at `/usr/share`, `/usr/local/share`, XDG data/config paths, `/etc`, `/usr/local/etc`, `~/.config`, `~/.pastebin.d`, and the executable-adjacent directory.
- Support legacy XML preferences through the 1.9.0 release: `pastebin`, `author`, `format`, `private`, and `expiry`.
- Preserve the `pastebinit` gettext domain and ship the reference `.po` catalog translations as compiled `.mo` files.
- Use local fixtures, not public pastebin services, for automated tests.
- The public release sequence is `v1.9.0-rc.1` then `v1.9.0`; Debian versions are `1.9.0~rc.1-1` then `1.9.0-1`.
- `1.9.0~rc.1-1` and `1.9.0-1` must sort after the reference Debian package `1.8.0-1`.
- The same-name Debian package is an in-place upgrade; never add `Conflicts` or `Replaces` referring to `pastebinit` itself.
- Run formatting, linting, unit tests, integration tests, differential tests, dependency security checks, Debian builds, installation smoke tests, and license/package metadata review before either public release.

---

## Planned File Structure

```text
Cargo.toml                         # Package metadata, Rust dependencies, and two binaries.
Cargo.lock                         # Locked, reviewed dependency graph.
LICENSE                            # GPL-2.0-or-later license text retained from reference.
README.md                          # Project identity, compatibility statement, install/use guidance.
src/lib.rs                         # Public module declarations and shared version constant.
src/error.rs                       # Typed errors and process exit-code policy.
src/cli.rs                         # Exact primary-client argument parsing and help/version rendering.
src/platform.rs                    # Distro detection and all 1.8.0 configuration search paths.
src/config.rs                      # INI parser, BasicInterpolation, and ordered site catalog.
src/preferences.rs                 # Legacy XML parsing and option-precedence resolution.
src/input.rs                       # Text input loading, Python-compatible trailing trim, echo output.
src/posting.rs                     # Payload generation and HTTP request planning.
src/transport.rs                   # Blocking HTTP submission and reference-style response URL parsing.
src/i18n.rs                        # gettext setup and translated diagnostic lookup.
src/helpers/mod.rs                 # Shared pbput command types and process helpers.
src/helpers/codec.rs               # Streaming GNU lzma/base64 subprocess pipeline.
src/helpers/pbput.rs               # pbput workflow.
src/bin/pastebinit.rs              # Primary executable process boundary.
src/bin/pbput.rs                   # pbput process boundary.
pastebin.d/*.conf                  # Exact 13 shipped 1.8.0 site definitions.
po/*.po                            # Imported 1.8.0 gettext catalogs.
po/Makefile                        # Deterministic .po to .mo compilation.
man/pastebinit.1                   # Primary command manual page.
man/pbput.1                        # Manual page for the installed pbput helper.
tests/reference/pastebinit-1.8.0/  # Unmodified, versioned 1.8.0 behavioral test fixture.
tests/support/mod.rs               # Test-only process, filesystem, and environment helpers.
tests/support/http_fixture.rs      # Local response server and request recorder.
tests/support/reference.rs         # Reference/Rust command runners and result comparator.
tests/unit_*.rs                    # Focused module tests.
tests/e2e_pastebinit.rs            # Hermetic primary-client process tests.
tests/differential_pastebinit.rs   # Reference versus Rust primary-client behavior matrix.
tests/e2e_pbput.rs                 # pbput integration and round-trip tests.
tests/differential_pbput.rs        # Reference versus Rust pbput behavior tests.
debian/control                     # Source/build/runtime dependency metadata.
debian/rules                       # dh-cargo build, test, localization build hooks.
debian/install                     # Installed binary, data, locale, and manpage paths.
debian/changelog                   # `1.9.0~rc.1-1` then `1.9.0-1` release history.
debian/copyright                   # DEP-5 copyright and GPL attribution.
debian/tests/smoke                 # Installed-package autopkgtest without network access.
.github/workflows/test.yml         # Formatting, linting, unit, integration, differential checks.
.github/workflows/package.yml      # Debian source/binary build and package smoke checks.
.github/workflows/release.yml      # Tag-only artifact, checksum, SBOM, and GitHub release workflow.
scripts/debian-version.sh          # SemVer-to-Debian version validation/mapping.
scripts/release-check.sh           # Reproducible local release gate command sequence.
docs/releasing.md                  # Human release candidate and stable publication procedure.
```

### Task 1: Bootstrap The Cargo Package And Import A Fixed Reference Fixture

**Files:**
- Create: `Cargo.toml`
- Create: `src/lib.rs`
- Create: `src/error.rs`
- Create: `LICENSE`
- Create: `tests/reference/README.md`
- Create: `tests/reference/pastebinit-1.8.0/pastebinit`
- Create: 13 files under `tests/reference/pastebinit-1.8.0/pastebin.d/`
- Create: `tests/reference/pastebinit-1.8.0/utils/pbput`
- Create: `tests/reference/pastebinit-1.8.0/utils/pbput.1`
- Create: `tests/unit_foundation.rs`
- Modify: `README.md`
- Modify: `.gitignore`

**Interfaces:**
- Produces: `pastebinit::VERSION: &str`, `pastebinit::AppError`, and `pastebinit::AppResult<T>` for every later Rust module.
- Produces: an executable, unmodified Python 1.8.0 fixture at `tests/reference/pastebinit-1.8.0/pastebinit` for all differential tests.
- Consumes: the source snapshot under `/home/maw/src/bin-inc/origin-sources/pastebinit-1.8.0`.

- [ ] **Step 1: Install the local build and test prerequisites if they are absent**

Run:

```bash
sudo apt-get update
sudo apt-get install --yes cargo rustc cargo-audit build-essential pkg-config libgettextpo-dev gettext mandoc python3 python3-distro tar xz-utils coreutils
```

Confirm the exact toolchain that will generate `Cargo.lock`:

```bash
cargo --version
rustc --version
python3 --version
```

The current workspace has no `cargo` executable, so do not begin Rust implementation until this step succeeds.

- [ ] **Step 2: Write the failing foundation test**

Create `tests/unit_foundation.rs`:

```rust
use pastebinit::{AppError, VERSION};

#[test]
fn version_and_operational_error_contract_are_stable() {
    assert_eq!(VERSION, "1.9.0-rc.1");
    assert_eq!(AppError::input("cannot read input").exit_code(), 1);
    assert_eq!(AppError::usage("invalid option").exit_code(), 2);
}
```

- [ ] **Step 3: Run the test to verify it fails before implementation**

Run:

```bash
cargo test --test unit_foundation
```

Expected: Cargo reports that the package or `pastebinit` library does not exist.

- [ ] **Step 4: Add package metadata and the minimal shared error interface**

Create `Cargo.toml` with the project version, two binary targets, explicit Rust edition, and locked dependencies:

```toml
[package]
name = "pastebinit"
version = "1.9.0-rc.1"
edition = "2024"
license = "GPL-2.0-or-later"
description = "A native Rust command-line pastebin client compatible with pastebinit 1.8.0"
repository = "https://github.com/bin-inc/pastebinit"
readme = "README.md"

[dependencies]
clap = { version = "4.5", features = ["derive"] }
gettextrs = { package = "gettext-rs", version = "0.7", features = ["gettext-system"] }
indexmap = "2.7"
quick-xml = "0.37"
regex = "1.11"
reqwest = { version = "0.12", default-features = false, features = ["blocking", "rustls-tls"] }
tempfile = "3.14"
signal-hook = "0.3"

[dev-dependencies]
assert_cmd = "2.0"

[[bin]]
name = "pastebinit"
path = "src/bin/pastebinit.rs"

[[bin]]
name = "pbput"
path = "src/bin/pbput.rs"

```

Create the declared binary files with `fn main() {}` so that Cargo can compile the package while later tasks replace the temporary stubs. Create `src/lib.rs` and `src/error.rs`:

```rust
// src/lib.rs
pub mod error;

pub use error::{AppError, AppResult};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
```

```rust
// src/error.rs
use std::fmt::Display;

pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug)]
pub struct AppError {
    message: String,
    exit_code: i32,
}

impl AppError {
    pub fn input(message: impl Display) -> Self {
        Self { message: message.to_string(), exit_code: 1 }
    }

    pub fn usage(message: impl Display) -> Self {
        Self { message: message.to_string(), exit_code: 2 }
    }

    pub fn exit_code(&self) -> i32 {
        1
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}
```

Use `cargo add` only if the resulting dependency features match the manifest above. Generate and retain `Cargo.lock` with `cargo generate-lockfile`.

- [ ] **Step 5: Import the immutable behavioral reference fixture**

Copy, do not move or edit, the Python executable and every shipped site definition:

```bash
REFERENCE_ROOT="/home/maw/src/bin-inc/origin-sources/pastebinit-1.8.0"
mkdir -p "tests/reference/pastebinit-1.8.0"
cp "$REFERENCE_ROOT/pastebinit" "tests/reference/pastebinit-1.8.0/pastebinit"
cp -a "$REFERENCE_ROOT/pastebin.d" "tests/reference/pastebinit-1.8.0/pastebin.d"
mkdir -p "tests/reference/pastebinit-1.8.0/utils"
cp "$REFERENCE_ROOT/utils/pbput" "tests/reference/pastebinit-1.8.0/utils/pbput"
cp "$REFERENCE_ROOT/utils/pbput.1" "tests/reference/pastebinit-1.8.0/utils/pbput.1"
chmod 0755 "tests/reference/pastebinit-1.8.0/pastebinit"
chmod 0755 "tests/reference/pastebinit-1.8.0/utils/pbput"
sha256sum "$REFERENCE_ROOT/pastebinit" "tests/reference/pastebinit-1.8.0/pastebinit" "$REFERENCE_ROOT/utils/pbput" "tests/reference/pastebinit-1.8.0/utils/pbput" "$REFERENCE_ROOT/utils/pbput.1" "tests/reference/pastebinit-1.8.0/utils/pbput.1"
```

Write `tests/reference/README.md` naming the exact source path, version `1.8.0`, GPL-2.0-or-later status, import date, and the matching SHA-256 values from the last command. The document must state that changes to this fixture require importing a newer reference release rather than editing the copy.

Copy the reference `COPYING` text to `LICENSE`, expand `README.md` to identify the project as an independently maintained Rust implementation compatible with 1.8.0, and add these generated paths to `.gitignore`:

```gitignore
po/mo/
*.deb
*.build
*.buildinfo
*.changes
```

- [ ] **Step 6: Run the foundation test and package checks**

Run:

```bash
cargo fmt --check
cargo test --test unit_foundation
cargo check --all-targets
test -x tests/reference/pastebinit-1.8.0/pastebinit
test -x tests/reference/pastebinit-1.8.0/utils/pbput
test -f tests/reference/pastebinit-1.8.0/utils/pbput.1
diff -u /home/maw/src/bin-inc/origin-sources/pastebinit-1.8.0/pastebinit tests/reference/pastebinit-1.8.0/pastebinit
diff -u /home/maw/src/bin-inc/origin-sources/pastebinit-1.8.0/utils/pbput tests/reference/pastebinit-1.8.0/utils/pbput
diff -u /home/maw/src/bin-inc/origin-sources/pastebinit-1.8.0/utils/pbput.1 tests/reference/pastebinit-1.8.0/utils/pbput.1
```

Expected: the Rust test passes, both temporary-stub binaries compile, and the reference executable has no diff.

- [ ] **Step 7: Commit the independently buildable foundation**

```bash
git add Cargo.toml Cargo.lock LICENSE README.md .gitignore src tests
git commit -m "build: bootstrap Rust pastebinit package"
```

### Task 2: Build Hermetic Process And HTTP Fixture Test Support

**Files:**
- Create: `tests/support/mod.rs`
- Create: `tests/support/http_fixture.rs`
- Create: `tests/support/reference.rs`
- Create: `tests/unit_http_fixture.rs`

**Interfaces:**
- Consumes: `tests/reference/pastebinit-1.8.0/pastebinit` from Task 1.
- Produces: `FixtureServer`, `ResponseSpec`, `RecordedRequest`, `run_reference`, `run_rust`, and `ProcessResult` for later end-to-end and differential tests.

- [ ] **Step 1: Write the failing HTTP recording test**

Create `tests/unit_http_fixture.rs`:

```rust
mod support;

use support::http_fixture::{FixtureServer, ResponseSpec};

#[test]
fn fixture_records_a_form_post_and_returns_its_configured_redirect() {
    let server = FixtureServer::spawn(vec![ResponseSpec::redirect("/paste/42")]);
    let response = support::http_fixture::send_test_request(
        &server.url(), "POST", "/submit", b"content=hello",
    );

    assert_eq!(response, 302);
    let request = server.next_request();
    assert_eq!(request.method, "POST");
    assert_eq!(request.path, "/submit");
    assert_eq!(request.body, b"content=hello");
}
```

The test request uses a standard-library `TcpStream` helper in `tests/support/http_fixture.rs`; do not add a production HTTP client solely for test setup. The helper writes a complete `POST /submit HTTP/1.1` request with `Content-Length: 13` and parses the first response status line.

- [ ] **Step 2: Run the test to verify it fails**

Run:

```bash
cargo test --test unit_http_fixture
```

Expected: compilation fails because `tests/support/http_fixture.rs` and `FixtureServer` do not exist.

- [ ] **Step 3: Implement a deterministic local fixture server**

Create `tests/support/http_fixture.rs` with these exact public test types:

```rust
pub struct ResponseSpec {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    pub delay: std::time::Duration,
}

pub struct RecordedRequest {
    pub method: String,
    pub path: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

pub struct FixtureServer {
    authority: std::net::SocketAddr,
    requests: std::sync::mpsc::Receiver<RecordedRequest>,
}

impl ResponseSpec {
    pub fn text(status: u16, body: impl Into<Vec<u8>>) -> Self;
    pub fn redirect(location: &str) -> Self;
}

impl FixtureServer {
    pub fn spawn(responses: Vec<ResponseSpec>) -> Self;
    pub fn url(&self) -> String;
    pub fn authority(&self) -> String;
    pub fn next_request(&self) -> RecordedRequest;
}

pub fn send_test_request(url: &str, method: &str, path: &str, body: &[u8]) -> u16;
```

Bind `TcpListener` to `127.0.0.1:0`, handle requests in a dedicated thread, read headers through `\r\n\r\n`, then read exactly `Content-Length` bytes, preserve received header/value bytes as UTF-8-lossy strings for assertions, and send each queued `ResponseSpec` in FIFO order. `authority()` returns `127.0.0.1:<allocated-port>` and `url()` returns `http://` plus that authority. Implement `send_test_request` with `TcpStream`: write a complete HTTP/1.1 request with the supplied `Content-Length`, read the status line, and return its numeric status. Set finite read timeouts so malformed tests do not hang. `ResponseSpec::redirect` must produce status 302, `Location`, `Content-Length: 0`, and no body.

Create `tests/support/reference.rs` with a process result that does not normalize output:

```rust
pub struct ProcessResult {
    pub code: Option<i32>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

pub fn run_reference(args: &[&str], stdin: &[u8], env: &[(String, String)]) -> ProcessResult;
pub fn run_rust(binary: &str, args: &[&str], stdin: &[u8], env: &[(String, String)]) -> ProcessResult;
pub fn assert_same_process_result(reference: &ProcessResult, rust: &ProcessResult);
pub struct IsolatedEnvironment {
    root: tempfile::TempDir,
    variables: Vec<(String, String)>,
}
pub fn isolated_environment_with_site(basename: &str, definition: &str) -> IsolatedEnvironment;
pub fn isolated_environment_with_reference_catalog() -> IsolatedEnvironment;
impl IsolatedEnvironment {
    pub fn variables(&self) -> Vec<(String, String)>;
}
```

`run_reference` invokes a temporary copy of `tests/reference/pastebinit-1.8.0/pastebinit` placed in an otherwise empty temporary executable directory, so its executable-adjacent `pastebin.d` lookup cannot load the fixture catalog. `run_rust` invokes the Cargo-built executable through `assert_cmd::cargo::cargo_bin`. Both runners must clear inherited `HOME`, `XDG_DATA_DIRS`, `XDG_CONFIG_DIRS`, and `XDG_CONFIG_HOME` before applying the supplied isolated environment. `isolated_environment_with_site` creates a temporary home/XDG tree, writes the supplied definition to `XDG_CONFIG_HOME/pastebin.d/local.conf`, and returns the exact environment entries including fixed `LC_ALL=C`, `LANG=C`, `TZ=UTC`, `USER=reference-user`, and `LOGNAME=reference-login`. `isolated_environment_with_reference_catalog` copies the 13 fixture definitions into that same XDG path so both executable locations see the identical catalog. `assert_same_process_result` compares exit code and raw stdout/stderr bytes and prints a UTF-8-lossy diff on failure.

Expose both modules from `tests/support/mod.rs`.

- [ ] **Step 4: Run the fixture test and ensure the test support compiles for later use**

Run:

```bash
cargo test --test unit_http_fixture
cargo test --no-run
```

Expected: the recording test passes and all existing test binaries compile.

- [ ] **Step 5: Commit the hermetic test infrastructure**

```bash
git add tests/support tests/unit_http_fixture.rs
git commit -m "test: add local HTTP and reference process fixtures"
```

### Task 3: Reproduce Primary CLI Parsing And Rendering

**Files:**
- Create: `src/cli.rs`
- Modify: `src/lib.rs`
- Modify: `src/bin/pastebinit.rs`
- Create: `tests/unit_cli.rs`

**Interfaces:**
- Consumes: `AppError`, `AppResult`, and `VERSION` from Task 1.
- Produces: `CliOptions`, `HelpDefaults`, `CliOutcome`, `parse_cli`, and `render_help` for Tasks 6 and 10.

- [ ] **Step 1: Record stable reference CLI cases, then write failing snapshots**

Run the imported reference in the C locale and capture these cases into test constants: `-h`, `-v`, `-l`, no arguments, one positional file, repeated positional files, `-i one two`, `-E`, every optional field, `-P 0`, unknown flags, and `-i` without an argument.

Create tests that assert the parsed values and the generated help/version actions:

```rust
use pastebinit::cli::{parse_cli, CliOutcome, HelpDefaults};

#[test]
fn parses_multiple_i_files_and_optional_posting_fields() {
    let outcome = parse_cli(
        ["pastebinit", "-i", "a", "b", "-a", "Ada", "-P", "0", "-E"],
        &HelpDefaults::reference_defaults("bpa.st", "maw"),
    )
    .unwrap();

    let CliOutcome::Run(options) = outcome else { panic!("expected run"); };
    assert_eq!(options.files, vec!["a", "b"]);
    assert_eq!(options.author.as_deref(), Some("Ada"));
    assert_eq!(options.private.as_deref(), Some("0"));
    assert!(options.echo);
}
```

- [ ] **Step 2: Run the CLI test to verify it fails**

Run:

```bash
cargo test --test unit_cli
```

Expected: compilation fails because the `cli` module and its public types do not exist.

- [ ] **Step 3: Implement the exact primary CLI contract**

Define the public types in `src/cli.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliOptions {
    pub files: Vec<String>,
    pub website: Option<String>,
    pub list_pastebins: bool,
    pub echo: bool,
    pub verbose: bool,
    pub author: Option<String>,
    pub title: Option<String>,
    pub format: Option<String>,
    pub private: Option<String>,
    pub expiry: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
}

pub struct HelpDefaults {
    website: String,
    author: String,
    format: String,
    private: String,
    expiry: String,
}

pub enum CliOutcome {
    Run(CliOptions),
    Help(String),
    Version(String),
}

pub fn parse_cli<I, T>(args: I, defaults: &HelpDefaults) -> AppResult<CliOutcome>
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone;

pub fn render_help(defaults: &HelpDefaults) -> String;
```

Use `clap` only for token parsing. Disable its generated help/version output and render `-h` and `-v` yourself so the option spelling, headings, default substitutions, and trailing newlines can be matched to the Python reference. Translate every invalid-argument error into `AppError::usage` so the executable exits 2, matching `argparse`; compare exact diagnostics in Task 10. Accept only the reference short flags: `-b`, `-i`, `-l`, `-E`, `-h`, `-v`, `-V`, `-a`, `-t`, `-f`, `-P`, `-e`, `-u`, and `-p`; do not add long aliases. Ensure `-i` consumes one or more files, positional file values append to the same `files` vector, `-P` preserves the string `"0"`, and `-l` produces a run action so Task 10 can list discovered sites.

The temporary binary must parse `std::env::args_os()`, print `CliOutcome::Help` or `CliOutcome::Version` to stdout, print `AppError::message()` to stderr, and return `AppError::exit_code()`.

- [ ] **Step 4: Compare the renderer against the reference and run the focused suite**

Run:

```bash
LC_ALL=C python3 tests/reference/pastebinit-1.8.0/pastebinit -h > /tmp/reference-help.txt
cargo run --quiet --bin pastebinit -- -h > /tmp/rust-help.txt
diff -u /tmp/reference-help.txt /tmp/rust-help.txt
cargo test --test unit_cli
```

Expected: after implementation, the help diff contains no differences except the version line, which is asserted separately as `pastebinit v1.9.0-rc.1`.

- [ ] **Step 5: Commit CLI parsing before configuration behavior is added**

```bash
git add src/cli.rs src/lib.rs src/bin/pastebinit.rs tests/unit_cli.rs
git commit -m "feat: add compatible pastebinit CLI parsing"
```

### Task 4: Implement Distribution Defaults And Exact Configuration Path Discovery

**Files:**
- Create: `src/platform.rs`
- Modify: `src/lib.rs`
- Create: `tests/unit_platform.rs`

**Interfaces:**
- Consumes: `AppResult` from Task 1.
- Produces: `RuntimeEnvironment`, `default_site_for_distro`, `pastebin_config_dirs`, and `xml_preference_paths` for Tasks 5, 6, and 10.

- [ ] **Step 1: Write failing path-order and distribution-default tests**

Create `tests/unit_platform.rs`:

```rust
use pastebinit::platform::{default_site_for_distro, pastebin_config_dirs, RuntimeEnvironment};

#[test]
fn distro_defaults_match_pastebinit_1_8_0() {
    assert_eq!(default_site_for_distro(Some("debian")), "paste.debian.net");
    assert_eq!(default_site_for_distro(Some("rocky")), "paste.centos.org");
    assert_eq!(default_site_for_distro(Some("opensuse-tumbleweed")), "paste.opensuse.org");
    assert_eq!(default_site_for_distro(None), "bpa.st");
}

#[test]
fn config_directories_follow_reference_precedence() {
    let env = RuntimeEnvironment::for_test("/home/alice", "/data/a:/data/b", "/cfg/a:/cfg/b", "/cfg/home");
    let dirs = pastebin_config_dirs(&env, "/opt/pastebinit/pastebinit");
    assert_eq!(dirs[0].to_string_lossy(), "/usr/share/pastebin.d");
    assert_eq!(dirs.last().unwrap().to_string_lossy(), "/opt/pastebinit/pastebin.d");
    assert!(dirs.iter().position(|p| p == std::path::Path::new("/data/b/pastebin.d"))
        < dirs.iter().position(|p| p == std::path::Path::new("/data/a/pastebin.d")));
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run:

```bash
cargo test --test unit_platform
```

Expected: compilation fails because `platform` has not been declared.

- [ ] **Step 3: Implement environment capture and reference ordering**

Implement these interfaces in `src/platform.rs`:

```rust
pub struct RuntimeEnvironment {
    pub home: std::path::PathBuf,
    pub xdg_data_dirs: String,
    pub xdg_config_dirs: String,
    pub xdg_config_home: std::path::PathBuf,
}

impl RuntimeEnvironment {
    pub fn from_process() -> Self;
    pub fn for_test(home: &str, data_dirs: &str, config_dirs: &str, config_home: &str) -> Self;
}

pub fn default_site_for_distro(distro_id: Option<&str>) -> &'static str;
pub fn detected_distro_id(os_release_path: &std::path::Path) -> Option<String>;
pub fn pastebin_config_dirs(env: &RuntimeEnvironment, executable: impl AsRef<std::path::Path>) -> Vec<std::path::PathBuf>;
pub fn xml_preference_paths(env: &RuntimeEnvironment) -> Vec<std::path::PathBuf>;
```

Parse the `ID=` field from `/etc/os-release`; map `debian` and `raspbian` to `paste.debian.net`, `fedora`, `centos`, `rhel`, and `rocky` to `paste.centos.org`, and any lower-case ID containing `suse` or equal to `sles` to `paste.opensuse.org`. Other IDs and unreadable files select `bpa.st`.

Build pastebin directories in this exact order: `/usr/share`, `/usr/local/share`, reversed colon-split `XDG_DATA_DIRS`, reversed colon-split `XDG_CONFIG_DIRS`, `/etc`, `/usr/local/etc`, `XDG_CONFIG_HOME`, `~/.pastebin.d`, then the canonical executable parent. Append `pastebin.d` unless an input already ends with `pastebin.d`; retain an empty XDG entry as relative `pastebin.d`, because the reference does. Remove duplicate final paths while retaining their first occurrence. Build XML paths from reversed `XDG_CONFIG_DIRS`, `/etc`, `/usr/local/etc`, `XDG_CONFIG_HOME`, and `~/.pastebinit.xml`, appending `pastebinit.xml` where necessary.

- [ ] **Step 4: Run focused tests including empty-XDG and duplicate-path cases**

Run:

```bash
cargo test --test unit_platform
```

Add and pass explicit tests for `XDG_DATA_DIRS=""`, an already-suffixed `pastebin.d` path, duplicate paths, and `XDG_CONFIG_HOME` unset falling back to `~/.config`.

- [ ] **Step 5: Commit platform compatibility behavior**

```bash
git add src/platform.rs src/lib.rs tests/unit_platform.rs
git commit -m "feat: add pastebinit configuration path discovery"
```

### Task 5: Parse And Overlay `pastebin.d` Definitions

**Files:**
- Create: `src/config.rs`
- Modify: `src/lib.rs`
- Create: `tests/unit_config.rs`
- Create: fixture `.conf` files under `tests/fixtures/config/`

**Interfaces:**
- Consumes: `RuntimeEnvironment` paths from Task 4 and `AppError` from Task 1.
- Produces: `SiteDefinition`, `SiteCatalog`, `load_site_catalog`, and `resolve_site` for Tasks 6, 8, and 10.

- [ ] **Step 1: Write failing INI and overlay tests**

Create `tests/unit_config.rs` with fixtures that exercise 1.8.0 semantics:

```rust
use pastebinit::config::load_site_catalog;

#[test]
fn later_definition_replaces_the_complete_site_definition() {
    let mut diagnostics = Vec::new();
    let catalog = load_site_catalog(&[
        "tests/fixtures/config/system/pastebin.d".into(),
        "tests/fixtures/config/user/pastebin.d".into(),
    ], &mut diagnostics)
    .unwrap();

    let site = catalog.get("example.test").unwrap();
    assert_eq!(site.pastebin("post_page"), Some("user-submit"));
    assert_eq!(site.format.get("content").map(String::as_str), Some("body"));
    assert_eq!(site.format.get("title"), None);
}

#[test]
fn target_page_unescapes_configparser_percent_escape() {
    let mut diagnostics = Vec::new();
    let catalog = load_site_catalog(&["tests/fixtures/config/interpolation/pastebin.d".into()], &mut diagnostics).unwrap();
    assert_eq!(catalog.get("json.test").unwrap().pastebin("target_page"), Some("show/%s/"));
}
```

The fixture definitions must include uppercase option names to prove reference-style lowercasing, a missing `[pastebin]` section, a missing `basename`, a `https = False` nonempty value, and `target_page = show/%%s/`.

- [ ] **Step 2: Run the parser tests to verify they fail**

Run:

```bash
cargo test --test unit_config
```

Expected: compilation fails because `config` and `load_site_catalog` do not exist.

- [ ] **Step 3: Implement ordered, ConfigParser-compatible site definitions**

Implement these public types and functions:

```rust
use indexmap::IndexMap;

pub struct SiteDefinition {
    pub basename: String,
    pub pastebin: IndexMap<String, String>,
    pub format: IndexMap<String, String>,
    pub defaults: IndexMap<String, String>,
    pub source: std::path::PathBuf,
}

impl SiteDefinition {
    pub fn pastebin(&self, key: &str) -> Option<&str>;
}

pub struct SiteCatalog {
    entries: IndexMap<String, SiteDefinition>,
}

impl SiteCatalog {
    pub fn get(&self, basename: &str) -> Option<&SiteDefinition>;
    pub fn names_sorted(&self) -> Vec<&str>;
}

pub fn load_site_catalog(directories: &[std::path::PathBuf], diagnostics: &mut dyn std::io::Write) -> AppResult<SiteCatalog>;
pub fn resolve_site<'a>(catalog: &'a SiteCatalog, raw_website: &str) -> AppResult<(&'a SiteDefinition, String)>;
```

Read only non-hidden `*.conf` files. Skip a file that cannot be decoded as text, as `ConfigParser.read()` does for `UnicodeError`. For a readable file with no `[pastebin]` or no `basename`, write the reference diagnostic containing its filename to the supplied diagnostics sink and skip it. Normalize option names to lowercase, retain declaration order using `IndexMap`, and implement BasicInterpolation sufficient for every reference configuration: `%%` becomes `%`; `%(key)s` resolves the supplied variable map before `[defaults]`; missing interpolation keys are an error.

For each valid file, replace the entire prior catalog entry at the same `basename`; do not merge sections from different files. `resolve_site` must remove a leading scheme with the same `://` split behavior and strip leading/trailing `/`, then construct `http://<basename>/` when `https` is absent or empty and `https://<basename>/` whenever its raw value is nonempty, including `https = False`.

- [ ] **Step 4: Run the parser suite and compare the shipped reference definitions**

Run:

```bash
cargo test --test unit_config
python3 tests/reference/pastebinit-1.8.0/pastebinit -l | LC_ALL=C sort > /tmp/reference-sites.txt
```

Add a test loading the copied reference `pastebin.d` directory and asserting exactly 13 names, including `bpa.st`, `paste.debian.net`, `paste.opendev.org`, and `sprunge.us`. For the command-level `-l` comparison in Task 10, use `isolated_environment_with_reference_catalog()` so Python and Rust load the same definitions rather than their executable-adjacent directories.

- [ ] **Step 5: Commit ordered site-configuration support**

```bash
git add src/config.rs src/lib.rs tests/unit_config.rs tests/fixtures/config
git commit -m "feat: load compatible pastebin definitions"
```

### Task 6: Support Legacy XML Preferences And Reference Option Precedence

**Files:**
- Create: `src/preferences.rs`
- Modify: `src/lib.rs`
- Create: `tests/unit_preferences.rs`
- Create: fixture `.xml` files under `tests/fixtures/preferences/`

**Interfaces:**
- Consumes: `CliOptions` from Task 3, `SiteDefinition` from Task 5, and XML paths from Task 4.
- Produces: `UserPreferences`, `InlineDefaults`, `load_preferences`, `help_defaults`, and `effective_option` for Tasks 8 and 10.

- [ ] **Step 1: Write failing XML lookup and precedence tests**

Create `tests/unit_preferences.rs`:

```rust
use pastebinit::preferences::{effective_option, load_preferences, InlineDefaults, UserPreferences};

#[test]
fn later_xml_file_and_cli_value_override_earlier_sources() {
    let preferences = load_preferences(&[
        "tests/fixtures/preferences/etc/pastebinit.xml".into(),
        "tests/fixtures/preferences/home/pastebinit.xml".into(),
    ])
    .unwrap();
    assert_eq!(preferences.author.as_deref(), Some("home author"));

    let cli = UserPreferences { author: Some("cli author".into()), ..UserPreferences::default() };
    assert_eq!(effective_option("user", &cli, &preferences, &InlineDefaults::for_user("alice")), "cli author");
}

#[test]
fn empty_xml_values_are_preserved_not_discarded() {
    let preferences = load_preferences(&["tests/fixtures/preferences/empty/pastebinit.xml".into()]).unwrap();
    assert_eq!(preferences.expiry.as_deref(), Some(""));
}
```

- [ ] **Step 2: Run the preferences test to verify it fails**

Run:

```bash
cargo test --test unit_preferences
```

Expected: compilation fails because the preferences module does not exist.

- [ ] **Step 3: Implement reference XML behavior and precedence**

Implement:

```rust
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UserPreferences {
    pub website: Option<String>,
    pub author: Option<String>,
    pub format: Option<String>,
    pub private: Option<String>,
    pub expiry: Option<String>,
    pub title: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
}

pub struct InlineDefaults {
    user: String,
    title: String,
    format: String,
    private: String,
    expiry: String,
    username: String,
    password: String,
}

impl InlineDefaults {
    pub fn for_user(user: &str) -> Self;
}

pub fn load_preferences(paths: &[std::path::PathBuf]) -> AppResult<UserPreferences>;
pub fn cli_preferences(options: &CliOptions) -> UserPreferences;
pub fn merge_preferences(xml: UserPreferences, cli: UserPreferences) -> UserPreferences;
pub fn help_defaults(default_site: &str, inline: &InlineDefaults, xml: &UserPreferences) -> HelpDefaults;
pub fn effective_option(key: &str, cli: &UserPreferences, xml: &UserPreferences, inline: &InlineDefaults) -> String;
```

For each existing XML file in path order, parse the whole document; malformed XML must produce the reference configuration error and terminate rather than silently skip it. Read the first text child of each `pastebin`, `author`, `format`, `private`, and `expiry` element. Later XML files replace earlier values. Apply CLI inputs last, so the effective precedence is CLI option, XML preference, site `[defaults]` value when Task 8 builds each configured parameter, then inline defaults. Inline defaults are the reference values: user is the value of `USER` when that variable exists, otherwise the value of `LOGNAME`; title `""`; format `"text"`; private `"1"`; expiry `""`; username `""`; password `""`.

Do not add XML support for title, username, or password: the struct carries them only so CLI and downstream code use a single type. `help_defaults` must reproduce the Python quirk where the `-e` help default is the inline expiry rather than an XML expiry.

- [ ] **Step 4: Run focused tests and a differential XML smoke case**

Run:

```bash
cargo test --test unit_preferences
```

Add a process test that creates two XML files in reference search order, invokes both executables with `-h`, and verifies their author/format/private defaults use the later XML values.

- [ ] **Step 5: Commit legacy-preference support**

```bash
git add src/preferences.rs src/lib.rs tests/unit_preferences.rs tests/fixtures/preferences
git commit -m "feat: support legacy pastebinit XML preferences"
```

### Task 7: Reproduce Text Input, Empty-Input, Echo, And File Failure Behavior

**Files:**
- Create: `src/input.rs`
- Modify: `src/lib.rs`
- Create: `tests/unit_input.rs`

**Interfaces:**
- Consumes: `CliOptions` from Task 3 and `AppError` from Task 1.
- Produces: `InputDocument`, `python_rstrip`, and `read_documents` for Task 10.

- [ ] **Step 1: Write failing input-behavior tests**

Create `tests/unit_input.rs`:

```rust
use pastebinit::input::python_rstrip;

#[test]
fn strips_the_same_trailing_whitespace_as_python_str_rstrip() {
    assert_eq!(python_rstrip("hello \n\t\u{001c}"), "hello");
    assert_eq!(python_rstrip("  hello  "), "  hello");
}
```

Add temporary-file tests for: a nonempty file, `-` reading supplied stdin, empty input after trimming, a missing file, and a two-file list that preserves source order.

- [ ] **Step 2: Run the test to verify it fails**

Run:

```bash
cargo test --test unit_input
```

Expected: compilation fails because `input` has not been implemented.

- [ ] **Step 3: Implement reference-style document loading**

Implement these interfaces:

```rust
pub struct InputDocument {
    pub display_name: String,
    pub content: String,
}

pub fn python_rstrip(value: &str) -> &str;
pub fn read_documents(files: &[String], stdin: &mut dyn std::io::Read) -> AppResult<Vec<InputDocument>>;
```

When `files` is empty, treat it as one `-` input. For `-`, record display name `STDIN`; otherwise use the supplied filename. Decode source text as UTF-8 and use `python_rstrip` before empty validation. Match Python `str.rstrip()` for Unicode whitespace and include U+001C through U+001F, which Rust’s base whitespace predicate does not universally classify the same way. Return an error containing `Error reading from: '<file>'` for unreadable paths and `You are trying to send an empty document, exiting.` after any document trims to empty. Do not read later files after the first input error.

Echoing belongs to Task 10’s orchestration because the Python reference prints each accepted content value immediately after reading it; `InputDocument` must not write output itself.

- [ ] **Step 4: Run tests and verify source ordering**

Run:

```bash
cargo test --test unit_input
```

Expected: all unit tests pass, including the U+001C edge case and fail-fast missing-file case.

- [ ] **Step 5: Commit the isolated input behavior**

```bash
git add src/input.rs src/lib.rs tests/unit_input.rs
git commit -m "feat: add compatible paste input loading"
```

### Task 8: Build Python-Compatible POST URLs, Parameters, And Encoded Bodies

**Files:**
- Create: `src/posting.rs`
- Modify: `src/lib.rs`
- Create: `tests/unit_posting.rs`

**Interfaces:**
- Consumes: `SiteDefinition` from Task 5 and `UserPreferences`/`InlineDefaults` from Task 6.
- Produces: `UploadPlan`, `EncodedBody`, `build_upload_plan`, and `python_json_object` for Task 9.

- [ ] **Step 1: Write failing body and URL construction tests**

Create `tests/unit_posting.rs`:

```rust
use pastebinit::posting::{build_upload_plan, EncodedBody};

#[test]
fn form_payload_preserves_format_order_and_python_plus_encoding() {
    let site = SiteDefinition {
        basename: "example.test".into(),
        pastebin: IndexMap::from([
            ("basename".into(), "example.test".into()),
            ("https".into(), "True".into()),
            ("post_page".into(), "submit".into()),
        ]),
        format: IndexMap::from([
            ("user".into(), "author".into()),
            ("content".into(), "content".into()),
        ]),
        defaults: IndexMap::new(),
        source: "fixture.conf".into(),
    };
    let cli = UserPreferences { author: Some("Ada".into()), ..UserPreferences::default() };
    let plan = build_upload_plan(&site, &cli, &UserPreferences::default(), &InlineDefaults::for_user("alice"), "a b&c").unwrap();
    assert_eq!(plan.url, "https://example.test/submit");
    assert_eq!(plan.body, EncodedBody::Form(b"author=Ada&content=a+b%26c".to_vec()));
}

#[test]
fn json_payload_matches_python_json_dumps_spacing_and_ascii_escaping() {
    let site = SiteDefinition {
        basename: "json.test".into(),
        pastebin: IndexMap::from([
            ("basename".into(), "json.test".into()),
            ("https".into(), "True".into()),
            ("post_format".into(), "json".into()),
        ]),
        format: IndexMap::from([( "content".into(), "content".into() )]),
        defaults: IndexMap::new(),
        source: "fixture.conf".into(),
    };
    let plan = build_upload_plan(&site, &UserPreferences::default(), &UserPreferences::default(), &InlineDefaults::for_user("alice"), "cafe\u{00e9}").unwrap();
    assert_eq!(plan.body, EncodedBody::Json(b"{\"content\": \"caf\\u00e9\"}".to_vec()));
    assert_eq!(plan.content_type.as_deref(), Some("text/json"));
}
```

Add complete test-site definitions for `post_page`, `post_format`, `paste_regexp`, `target_page`, `user_length`, a site default, and an inline option override. Import `indexmap::IndexMap`, `pastebinit::config::SiteDefinition`, and `pastebinit::preferences::{InlineDefaults, UserPreferences}` in this test file.

- [ ] **Step 2: Run the posting tests to verify they fail**

Run:

```bash
cargo test --test unit_posting
```

Expected: compilation fails because `posting` and its types do not exist.

- [ ] **Step 3: Implement ordered parameter and body generation**

Implement:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EncodedBody {
    Form(Vec<u8>),
    Json(Vec<u8>),
}

pub struct UploadPlan {
    pub url: String,
    pub base_url: String,
    pub body: EncodedBody,
    pub content_type: Option<String>,
    pub response_pattern: Option<String>,
    pub target_url: Option<String>,
}

pub fn build_upload_plan(
    site: &SiteDefinition,
    cli: &UserPreferences,
    xml: &UserPreferences,
    inline: &InlineDefaults,
    content: &str,
) -> AppResult<UploadPlan>;

pub fn python_form_urlencode(parameters: &indexmap::IndexMap<String, String>) -> Vec<u8>;
pub fn python_json_object(parameters: &indexmap::IndexMap<String, String>) -> Vec<u8>;
```

Walk the `[format]` section in declaration order. Map `content` directly to document content. For every other format key, resolve `CLI > XML > [defaults] > inline` with BasicInterpolation variables overriding `[defaults]`, exactly like `ConfigParser.get(..., vars=spec_opts, fallback=inline)`. Preserve custom keys in `[format]` and `[defaults]`; do not restrict them to the known CLI fields.

Construct the posting URL by appending `post_page` with leading `/` removed to the resolved base URL. A missing `post_page` uses the base URL. Retain the resolved base URL in `UploadPlan` for Task 9's regex-result fallback. If `sizelimit` is present, compare the Rust Unicode scalar count to Python `len(content)` and fail before any request. If `user_length` is present, calculate the effective user then truncate to `limit - 1` characters and append `~` only when it exceeds the limit, matching the reference.

Do not use generic serializers without byte-level tests: Python `urlencode` uses `+` for space and Python `json.dumps` uses insertion order, spaces after commas/colons, and `ensure_ascii=True`. Implement the two encoder functions to reproduce those bytes. Select `EncodedBody::Json` only when `post_format == "json"`; otherwise select `EncodedBody::Form`. JSON has `Content-Type: text/json`; forms retain the standard `application/x-www-form-urlencoded` content type supplied in Task 9.

- [ ] **Step 4: Compare body bytes with Python through the local server**

Run:

```bash
cargo test --test unit_posting
```

Add tests for `bpa.st` JSON, `paste.opendev.org` JSON with `target_page = show/%%s/`, `paste.debian.net` user truncation, and a custom non-ASCII default such as `发送` from `paste.ubuntu.org.cn.conf`.

- [ ] **Step 5: Commit request planning and encoding**

```bash
git add src/posting.rs src/lib.rs tests/unit_posting.rs
git commit -m "feat: build compatible pastebin requests"
```

### Task 9: Submit HTTP Requests And Derive Result URLs

**Files:**
- Create: `src/transport.rs`
- Modify: `src/lib.rs`
- Create: `tests/unit_transport.rs`

**Interfaces:**
- Consumes: `UploadPlan` and `EncodedBody` from Task 8 and `FixtureServer` from Task 2.
- Produces: `Transport`, `ReqwestTransport`, `HttpResponse`, `submit_upload`, and `extract_paste_url` for Task 10.

- [ ] **Step 1: Write failing redirect and regex extraction tests**

Create `tests/unit_transport.rs`:

```rust
use pastebinit::posting::{EncodedBody, UploadPlan};
use pastebinit::transport::extract_paste_url;

fn plan(pattern: Option<&str>, target_url: Option<&str>) -> UploadPlan {
    UploadPlan {
        url: "https://example.test/submit".into(),
        base_url: "https://example.test/".into(),
        body: EncodedBody::Form(Vec::new()),
        content_type: None,
        response_pattern: pattern.map(str::to_owned),
        target_url: target_url.map(str::to_owned),
    }
}

#[test]
fn capturing_regex_uses_python_re_split_index_one_semantics() {
    assert_eq!(
        extract_paste_url(&plan(Some(r#""data": "([^"]+)""#), Some("https://example.test/show/%s/")), "ignored", br#"{"data": "abc"}"#).unwrap(),
        "https://example.test/show/abc/"
    );
}

#[test]
fn direct_pattern_returns_the_entire_utf8_body() {
    assert_eq!(extract_paste_url(&plan(Some("(.*)"), None), "ignored", b"https://paste.test/42\n").unwrap(), "https://paste.test/42");
}
```

Add server-backed tests for form and JSON headers, `User-Agent: Pastebinit v1.9.0-rc.1`, a 302 final URL, a closed socket, invalid UTF-8 response data, and a response that does not match its configured regex.

- [ ] **Step 2: Run the transport test to verify it fails**

Run:

```bash
cargo test --test unit_transport
```

Expected: compilation fails because `transport` is missing.

- [ ] **Step 3: Implement blocking transport and reference response interpretation**

Implement the following interface:

```rust
pub struct HttpResponse {
    pub final_url: String,
    pub body: Vec<u8>,
}

pub trait Transport {
    fn post(&self, plan: &UploadPlan) -> AppResult<HttpResponse>;
}

pub struct ReqwestTransport {
    client: reqwest::blocking::Client,
}

impl ReqwestTransport {
    pub fn new() -> AppResult<Self>;
}

pub fn submit_upload(transport: &dyn Transport, plan: &UploadPlan) -> AppResult<String>;
pub fn extract_paste_url(plan: &UploadPlan, final_url: &str, body: &[u8]) -> AppResult<String>;
```

Configure one blocking `reqwest` client with a 15-second connect/read deadline and normal redirects. Add `User-Agent: Pastebinit v<package-version>` only when the request has not already specified one. Send forms with `application/x-www-form-urlencoded` and JSON with exactly `text/json`. Call `error_for_status()` so HTTP status failures follow Python `urlopen` failure behavior rather than being treated as success. Convert transport failures into `Failed to contact the server: <error>` at the process boundary in Task 10. For the local status/connection scenarios, map the error kind to the Python 1.8.0 formatting captured by the reference runner and assert the full resulting diagnostic byte sequence.

If no `paste_regexp` exists, return the final URL after redirects. If it is exactly `(.*)`, UTF-8-decode, trim using Python-compatible trailing stripping, and return the entire body. Otherwise emulate `re.split(pattern, body)[1]`, not merely Rust `Regex::split`: when the first regex match has capturing groups, index one is capture group one; with no capture groups, index one is the post-match substring. If a `target_page` exists, apply Python string `%` formatting to it with the extracted string exactly as `relink_target_url % result` does; otherwise concatenate `plan.base_url` and the extracted text. Invalid UTF-8, compilation failure, no match, absent capture group, or invalid format replacement returns the reference-style result-page error.

- [ ] **Step 4: Run transport tests with recorded raw requests**

Run:

```bash
cargo test --test unit_transport
```

Expected: all tests pass; the fixture assertions prove raw form/JSON bodies are exactly those generated by Task 8.

- [ ] **Step 5: Commit HTTP transport behavior**

```bash
git add src/transport.rs src/lib.rs tests/unit_transport.rs
git commit -m "feat: submit pastebin requests and parse results"
```

### Task 10: Wire The `pastebinit` Binary And Establish The Differential Compatibility Matrix

**Files:**
- Modify: `src/bin/pastebinit.rs`
- Create: `tests/e2e_pastebinit.rs`
- Create: `tests/differential_pastebinit.rs`
- Create: differential fixture `.conf` files under `tests/fixtures/differential/`
- Create: differential fixture `.xml` files under `tests/fixtures/differential/`

**Interfaces:**
- Consumes: all primary-client modules from Tasks 3 through 9 and process support from Task 2.
- Produces: the fully functional `pastebinit` binary and reproducible primary-client differential test coverage.

- [ ] **Step 1: Write a failing end-to-end test for a local form upload**

Create `tests/e2e_pastebinit.rs`:

```rust
mod support;

#[test]
fn stdin_form_upload_prints_the_redirect_target() {
    let server = support::http_fixture::FixtureServer::spawn(vec![
        support::http_fixture::ResponseSpec::redirect("/p/42"),
        support::http_fixture::ResponseSpec::text(200, b"stored".to_vec()),
    ]);
    let environment = support::isolated_environment_with_site(
        &server.authority(),
        &format!("[pastebin]\nbasename = {}\npost_page = submit\n\n[format]\ncontent = body\n\n[defaults]\n", server.authority()),
    );

    let authority = server.authority();
    let result = support::reference::run_rust("pastebinit", &["-b", authority.as_str()], b"hello\n", &environment.variables());
    assert_eq!(result.code, Some(0));
    assert_eq!(result.stdout, format!("{}/p/42\n", server.url()).into_bytes());
}
```

Keep the fixture endpoint valid by using the fixture server's `127.0.0.1:<port>` authority as both configuration basename and `-b` argument. With no `https` setting, reference resolution constructs an `http://` base URL and the `post_page = submit` suffix yields `<fixture-url>/submit`. It must never contact a public hostname.

- [ ] **Step 2: Run the end-to-end test to verify it fails**

Run:

```bash
cargo test --test e2e_pastebinit
```

Expected: the placeholder `pastebinit` binary does not yet load configuration or submit input, so the test fails.

- [ ] **Step 3: Implement process orchestration without new compatibility policy**

Replace the placeholder `main` with an `execute` function that performs the reference sequence:

```rust
fn execute(
    argv: impl IntoIterator<Item = std::ffi::OsString>,
    stdin: &mut dyn std::io::Read,
    stdout: &mut dyn std::io::Write,
    stderr: &mut dyn std::io::Write,
) -> pastebinit::AppResult<()>;
```

Use the reference English source messages in this task; Task 11 will wrap the same source strings in the gettext translator without changing `LC_ALL=C` output. Discover runtime paths, determine the distribution default, load XML preferences before parsing CLI so help defaults match the reference, parse CLI, load the site catalog using stderr as its diagnostics sink, and handle `-l` by writing `Supported pastebins:` followed by sorted `- <basename>` lines. Resolve CLI/XML/default website, load documents, echo each accepted document before its upload if `-E` is set, build a plan, print exactly `POSTing to: <url>\nParams: <bytes-debug>` to stderr when `-V` is set, submit, and print the resulting URL. Stop immediately on the first failure.

Render errors in this binary with the reference English source strings from 1.8.0. Task 11 wraps those strings in gettext, but the byte sequence under `LC_ALL=C` must be stable now. Explicitly test and preserve stderr/stdout separation and one trailing newline per printed line.

- [ ] **Step 4: Add the full differential scenario table**

Create table-driven tests that run `run_reference` and `run_rust` in the same temporary home/XDG tree, then call `assert_same_process_result`. Cover all rows below; use fixture server request records to compare request bytes after asserting the process results:

| Scenario | Required assertion |
| --- | --- |
| `-h` and `-v` | Same help structure and streams; version text asserted as the respective package version. |
| `-l` | Same sorted 13-site list with copied definitions. |
| no input, stdin, positional file, `-i`, and multiple files | Same content trimming, output ordering, and one URL per input. |
| `-E` | Content then a URL-length hyphen line then URL, matching reference line order. |
| `-a`, `-t`, `-f`, `-P`, `-e`, `-u`, and `-p` | Same request fields and values. |
| XML then CLI overrides | Same selected site and parameter values. |
| system/XDG/home/executable configurations | Same definition override winner. |
| standard form, `bpa.st` JSON, and `paste.opendev.org` target-page | Same method, URL, headers, and raw body. |
| redirects, `(.*)`, capture regex, no capture regex | Same printed URL or result error. |
| unknown or malformed CLI flags, including missing option arguments | Same status 2, stdout, and stderr diagnostics. |
| missing site/config, empty input, unreadable input, size limit, bad XML, invalid response UTF-8, connection failure | Same status 1 and diagnostics. |
| `-V` | Same destination and byte-debug representation with version-specific user-agent exception documented in the assertion. |

For reference calls, install `python3-distro` in CI and set `LC_ALL=C`, `LANG=C`, `TZ=UTC`, fixed `USER`, and fixed `LOGNAME`. Use `isolated_environment_with_reference_catalog()` for all copied-site scenarios and `isolated_environment_with_site()` for isolated custom-site scenarios. Do not compare Python and Rust raw `-v` version text or User-Agent version byte-for-byte; assert each against its own declared version and compare all other behavior. Add a `-V -p secret` scenario that asserts both implementations expose the encoded password in stderr exactly as the reference does; document the risk in the README and `pastebinit(1)`.

- [ ] **Step 5: Run unit, end-to-end, and differential primary-client tests**

Run:

```bash
cargo test --test e2e_pastebinit
cargo test --test differential_pastebinit
cargo test
```

Expected: all matrix scenarios pass without outbound network access.

- [ ] **Step 6: Commit the primary command and compatibility suite**

```bash
git add src/bin/pastebinit.rs tests/e2e_pastebinit.rs tests/differential_pastebinit.rs tests/fixtures/differential
git commit -m "feat: complete compatible pastebinit command"
```

### Task 11: Ship Definitions, Localize Diagnostics, And Install Manual Pages

**Files:**
- Create: 13 shipped `.conf` files under `pastebin.d/`
- Create: `po/Makefile`
- Create: `src/i18n.rs`
- Modify: `src/lib.rs`
- Modify: `src/bin/pastebinit.rs`
- Create: `man/pastebinit.1`
- Create: `man/pbput.1`
- Modify: `README.md`
- Create: `tests/unit_i18n.rs`

**Interfaces:**
- Consumes: the 1.8.0 `pastebin.d`, `po/*.po`, `pastebinit.xml`, and `utils/pbput.1` reference files.
- Produces: `Translator`, `tr`, packageable definitions/catalogs/manpages, and user documentation for Tasks 15 and 16.

- [ ] **Step 1: Import data files and write failing localization tests**

Copy all 13 reference `.conf` files and all reference `.po` files. Do not alter keys, ordering, comments, or the Chinese default value in `paste.ubuntu.org.cn.conf`. Create a test that loads the shipped definition directory and asserts its complete sorted name list. Add a locale test:

```rust
use pastebinit::i18n::Translator;

#[test]
fn c_locale_retains_the_reference_english_message() {
    let translator = Translator::for_test("C", "po/mo");
    assert_eq!(translator.tr("Unknown website, please post a bugreport to request this pastebin to be added (%s)"),
        "Unknown website, please post a bugreport to request this pastebin to be added (%s)");
}
```

- [ ] **Step 2: Run the localization test to verify it fails**

Run:

```bash
cargo test --test unit_i18n
```

Expected: the translation module and compiled catalog directory are missing.

- [ ] **Step 3: Implement gettext setup and source-string inventory**

Implement:

```rust
pub struct Translator {
    domain: String,
    locale_dir: std::path::PathBuf,
}

impl Translator {
    pub fn system() -> Self;
    pub fn for_test(locale: &str, locale_dir: impl AsRef<std::path::Path>) -> Self;
    pub fn tr(&self, message: &str) -> String;
}
```

Bind the `pastebinit` domain to `/usr/share/locale` for an installed binary; test construction binds a supplied temporary locale directory. Replace every primary-client diagnostic literal in `src/bin/pastebinit.rs` with `translator.tr(reference_source_string)`. Keep each English source string byte-for-byte equal to the reference gettext input so imported catalogs remain usable. The `-V` diagnostic remains English if it is not a reference gettext string.

Create `po/Makefile`:

```make
PO_FILES := $(wildcard *.po)
MO_FILES := $(patsubst %.po,mo/%/LC_MESSAGES/pastebinit.mo,$(PO_FILES))

all: $(MO_FILES)

clean:
	rm -rf mo

mo/%/LC_MESSAGES/pastebinit.mo: %.po
	mkdir -p $(dir $@)
	msgfmt --check $< -o $@
```

Run `make -C po`, leave generated `.mo` files ignored, and test an available non-English catalog in addition to `C` only when the imported translation contains the selected string.

- [ ] **Step 4: Write package-ready manual pages and user documentation**

Create static roff pages. `man/pastebinit.1` must document every retained primary option, stdin/multiple-file semantics, the full configuration location precedence, all legacy XML values, `pastebin.d` sections and fields, exit behavior, and `bin-inc/pastebinit` provenance. Copy the reference `utils/pbput.1` unchanged to `man/pbput.1`, because exact Debian package parity includes its installed documentation even though it describes the uninstalled historical invocation aliases.

Update the README with build prerequisites, `cargo build --release`, compatible command examples, the installed data layout, current limitations of third-party service definitions, the fact that `-V` exposes encoded request parameters including supplied passwords, license attribution, and a statement that public services are not used by the test suite. Add the same `-V` credential-exposure warning to `man/pastebinit.1`.

- [ ] **Step 5: Verify data, catalogs, and manpages**

Run:

```bash
make -C po
cargo test --test unit_i18n
test "$(ls pastebin.d/*.conf | wc -l)" -eq 13
mandoc -T lint man/pastebinit.1 man/pbput.1
```

Expected: message catalogs compile with no fuzzy/format errors; manpages have no lint errors.

- [ ] **Step 6: Commit user-facing data and documentation**

```bash
git add pastebin.d po src/i18n.rs src/lib.rs src/bin/pastebinit.rs man README.md tests/unit_i18n.rs
git commit -m "feat: ship pastebin data translations and manuals"
```

### Task 12: Port The Installed `pbput` Command And Shared Binary Payload Transforms

**Files:**
- Create: `src/helpers/mod.rs`
- Create: `src/helpers/codec.rs`
- Create: `src/helpers/pbput.rs`
- Create: `src/bin/pbput.rs`
- Modify: `src/lib.rs`
- Create: `tests/e2e_pbput.rs`
- Create: `tests/differential_pbput.rs`

**Interfaces:**
- Consumes: `AppError` from Task 1 and the installed `pastebinit` command contract from Task 10.
- Produces: `run_pbput` and `HelperEnvironment` for Debian packaging.

- [ ] **Step 1: Write failing `pbput` transform and process tests**

Create tests that put a fake executable named `pastebinit` first on `PATH`. The fake reads stdin to a capture file, records arguments, and prints `https://paste.test/42`. Test that Rust `pbput` sends `paste.ubuntu.com` as the `-b` argument and that `base64 -d | lzma -d` over its captured bytes yields the original stdin bytes. Assert the encoded captured output contains a newline every 76 bytes, matching GNU `base64` defaults used by the reference pipeline. Add a large-input test that exceeds the default pipe buffer to prove the subprocess chain drains concurrently rather than accumulating transformed bytes in Rust memory.

Add a differential test that runs the imported reference `utils/pbput` under the same fake `pastebinit`, then compares exit code, stdout, captured argument list, and decoded payload to the Rust binary.

- [ ] **Step 2: Run helper tests to verify they fail**

Run:

```bash
cargo test --test e2e_pbput
cargo test --test differential_pbput
```

Expected: `pbput` binary and helper module do not exist.

- [ ] **Step 3: Implement codec and pbput workflow**

Implement these public interfaces:

```rust
pub struct HelperEnvironment {
    pub pastebinit_program: std::ffi::OsString,
    pub temp_parent: std::path::PathBuf,
}

pub fn run_pbput(
    args: &[std::ffi::OsString],
    stdin: &mut dyn std::io::Read,
    stdout: &mut dyn std::io::Write,
    stderr: &mut dyn std::io::Write,
    environment: &HelperEnvironment,
) -> AppResult<()>;
```

For a readable positional path, invoke system `tar cf <temporary-file> <path>` to preserve the reference archive representation. With no readable path, copy stdin into the temporary source file. Spawn the exact three-process transform/upload chain `lzma -9 -f -c <source> | base64 | pastebinit -b paste.ubuntu.com`, connecting each child through OS pipes and concurrently draining the final stdout/stderr into the supplied writers. This preserves GNU `base64` 76-column line wrapping, its trailing newline, bounded Rust memory use, and the reference shell pipeline's final `pastebinit` exit status. Do not add `set -e`-style early termination: if `tar`, `lzma`, or `base64` fails, preserve its diagnostic and allow the downstream stages to observe the pipe close, as the reference shell script does. Use `tempfile` and a cleanup guard so temp resources are removed on normal return and unwind; use `signal-hook` to translate `SIGINT`, `SIGHUP`, `SIGQUIT`, and `SIGTERM` into cleanup followed by nonzero exit in the binary wrapper.

Resolve `pastebinit` through `PATH`, matching the reference shell script's bare `pastebinit` invocation. The Rust helper owns all transforms; it does not ship or invoke the historical shell helper.

- [ ] **Step 4: Run stdin, file, directory, and reference differential checks**

Run:

```bash
cargo test --test e2e_pbput
cargo test --test differential_pbput
```

Add tests for a file and a directory input. Decode with system `base64 -d | lzma -d`, then inspect names and contents with system `tar`; do not compare archive bytes because tar metadata such as timestamps is not stable across independent runs. Add a fake `pastebinit` failure case and compare the reference and Rust resulting status/stdout/stderr, including diagnostics emitted by a deliberately failing transform executable placed first on `PATH`.

- [ ] **Step 5: Commit the pbput port**

```bash
git add src/helpers src/bin/pbput.rs src/lib.rs tests/e2e_pbput.rs tests/differential_pbput.rs
git commit -m "feat: port pbput binary uploads to Rust"
```

### Task 13: Build An Upgrade-Compatible Debian Package

**Files:**
- Create: `debian/control`
- Create: `debian/rules`
- Create: `debian/install`
- Create: `debian/changelog`
- Create: `debian/copyright`
- Create: `debian/source/format`
- Create: `debian/tests/control`
- Create: `debian/tests/smoke`
- Create: `tests/package_smoke.sh`

**Interfaces:**
- Consumes: both binaries, `pastebin.d`, generated `po/mo`, and `man/*.1` from Tasks 1 through 12.
- Produces: an architecture-dependent Debian package named `pastebinit` that upgrades version `1.8.0-1` and passes installed-command smoke tests.

- [ ] **Step 1: Write failing package-layout assertions**

Create `tests/package_smoke.sh`:

```sh
#!/bin/sh
set -eu

test -x /usr/bin/pastebinit
test -x /usr/bin/pbput
test -f /usr/share/pastebin.d/bpa.st.conf
test -f /usr/share/locale/de/LC_MESSAGES/pastebinit.mo
test -f /usr/share/man/man1/pastebinit.1.gz
expected_version=${EXPECTED_VERSION:-1.9.0-rc.1}
pastebinit -v | grep -Fx "pastebinit v$expected_version"
pastebinit -l | grep -Fx -- '- bpa.st'
```

Run it before packaging. Expected: it fails because no Debian package has installed the binaries or data paths.

- [ ] **Step 2: Add Debian metadata with the required version ordering**

Create `debian/changelog` beginning exactly:

```text
pastebinit (1.9.0~rc.1-1) unstable; urgency=medium

  * First release of the native Rust implementation compatible with pastebinit 1.8.0.

 -- Bin Inc <packages@bin-inc.example>  Sat, 05 Sep 2026 00:00:00 +0000
```

Replace the maintainer email with the actual release-maintainer address before publication. Create `debian/control` with source and binary package name `pastebinit`, `Architecture: any`, `Rules-Requires-Root: no`, and Homepage `https://github.com/bin-inc/pastebinit`. Set build dependencies to `debhelper-compat (= 13)`, `dh-cargo`, `cargo`, `rustc`, `gettext`, `pkg-config`, `libgettextpo-dev`, `librust-clap-dev`, `librust-gettext-rs-dev`, `librust-indexmap-dev`, `librust-quick-xml-dev`, `librust-regex-dev`, `librust-reqwest-dev`, `librust-tempfile-dev`, and `librust-signal-hook-dev`; add `librust-assert-cmd-dev` when the package build runs the full test suite. Include `${misc:Depends}`, `${shlibs:Depends}`, `tar`, `xz-utils`, `coreutils`, and `ca-certificates` runtime dependencies for the installed `pbput` pipeline. Before the first source upload, verify each listed crate package and required feature is available in the targeted Debian suite; if one is unavailable, pin Cargo dependencies to that suite's packaged crate version rather than downloading crates during the Debian build. Do not add `Conflicts`, `Breaks`, `Replaces`, or `Provides` for `pastebinit` itself.

Create `debian/copyright` in DEP-5 format that retains the source reference’s upstream, Debian packaging, helper, and translation copyright holders, adds Bin Inc copyright only for new Rust/packaging files, and supplies GPL-2+ text/location.

- [ ] **Step 3: Configure reproducible build and install paths**

Create executable `debian/rules`:

```make
#!/usr/bin/make -f

%:
	dh $@ --buildsystem=cargo

override_dh_auto_build:
	dh_auto_build
	$(MAKE) -C po

override_dh_auto_test:
	dh_auto_test -- test --locked
```

Create `debian/install`:

```text
target/release/pastebinit usr/bin/
target/release/pbput usr/bin/
pastebin.d/*.conf usr/share/pastebin.d/
po/mo/ usr/share/locale/
man/pastebinit.1 usr/share/man/man1/
man/pbput.1 usr/share/man/man1/
```

Before finalizing `debian/install`, build once with `dpkg-buildpackage -us -uc -b`, inspect its `dh_auto_build` output, and set both binary source paths to the exact deterministic artifact directory produced by `dh-cargo`; never copy artifacts from an ad-hoc developer `target/release` directory. The paths shown above are the expected default only.

Set `debian/source/format` to `3.0 (quilt)`. Create a no-network autopkgtest in `debian/tests/control` and `debian/tests/smoke` that calls `tests/package_smoke.sh` after package installation.

- [ ] **Step 4: Build, install, and smoke-test the generated package**

Run:

```bash
dpkg-buildpackage -us -uc -b
sudo apt-get install --yes ../pastebinit_1.9.0~rc.1-1_*.deb
tests/package_smoke.sh
dpkg --compare-versions 1.9.0~rc.1-1 gt 1.8.0-1
dpkg --compare-versions 1.9.0-1 gt 1.9.0~rc.1-1
```

Run the installation smoke test in a disposable Debian container or VM, not on a development system whose existing `/usr/bin/pastebinit` may be needed. Set `EXPECTED_VERSION=1.9.0-rc.1` for this candidate smoke run. Record `dpkg -L pastebinit` and verify every listed package-owned path is intended.

- [ ] **Step 5: Commit Debian packaging**

```bash
git add debian tests/package_smoke.sh
git commit -m "build: package pastebinit for Debian"
```

### Task 14: Automate Release Gates And Publish The 1.9.0 Candidate Then Stable Release

**Files:**
- Create: `.github/workflows/test.yml`
- Create: `.github/workflows/package.yml`
- Create: `.github/workflows/release.yml`
- Create: `scripts/debian-version.sh`
- Create: `scripts/release-check.sh`
- Create: `docs/releasing.md`
- Modify: `README.md`
- Modify: `debian/changelog`

**Interfaces:**
- Consumes: the complete implementation, differential tests, and Debian package from Tasks 1 through 13.
- Produces: reproducible CI evidence and a signed `v1.9.0-rc.1` followed by `v1.9.0` release with source, Debian, checksum, SBOM, and provenance artifacts.

- [ ] **Step 1: Write failing SemVer-to-Debian mapping tests**

Create a shell test in `scripts/release-check.sh` that requires:

```sh
test "$(scripts/debian-version.sh 1.9.0-rc.1)" = "1.9.0~rc.1-1"
test "$(scripts/debian-version.sh 1.9.0)" = "1.9.0-1"
dpkg --compare-versions "$(scripts/debian-version.sh 1.9.0-rc.1)" gt 1.8.0-1
dpkg --compare-versions "$(scripts/debian-version.sh 1.9.0)" gt "$(scripts/debian-version.sh 1.9.0-rc.1)"
```

- [ ] **Step 2: Run the mapping checks to verify they fail**

Run:

```bash
sh scripts/release-check.sh
```

Expected: the script does not exist.

- [ ] **Step 3: Implement local release verification scripts**

Create executable `scripts/debian-version.sh`:

```sh
#!/bin/sh
set -eu

version=${1:?pass a SemVer version without a leading v}
if ! printf '%s\n' "$version" | grep -Eq '^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(-rc\.(0|[1-9][0-9]*))?$'; then
    printf '%s\n' "invalid release version: $version" >&2
    exit 2
fi

case "$version" in
  *-rc.*) printf '%s~rc.%s-1\n' "${version%-rc.*}" "${version#*-rc.}" ;;
  *) printf '%s-1\n' "$version" ;;
esac
```

The release-candidate expansion must emit `1.9.0~rc.1-1` exactly. Add test cases for `rc.2`, leading-zero numeric components, unsupported prerelease labels, and build metadata. `scripts/release-check.sh` must run, in this order:

```sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --locked
cargo audit
make -C po clean all
mandoc -T lint man/pastebinit.1 man/pbput.1
dpkg-buildpackage -us -uc -b
```

Then install the produced `.deb` in a disposable Debian container, run `EXPECTED_VERSION="$(grep '^version = ' Cargo.toml | cut -d '"' -f 2)" tests/package_smoke.sh`, run `git diff --check`, and require a clean `git status --short` except for expected signed release artifacts outside the repository.

- [ ] **Step 4: Add GitHub Actions release gates**

Create `test.yml` for pull requests and pushes on Ubuntu 26.04: install Rust stable, `cargo-audit`, and the Task 1 test prerequisites; cache Cargo registry/build output keyed by `Cargo.lock`; run `cargo fmt --check`, Clippy with warnings denied, `cargo test --locked`, `cargo audit`, PO compilation, and manpage lint.

Create `package.yml` for pushes and release tags: run `dpkg-buildpackage -us -uc -b` in a `debian:trixie` container that first installs every declared Debian build dependency; install the resulting package in a fresh `debian:trixie` container, run the smoke test with `EXPECTED_VERSION` read from `Cargo.toml`, compare Debian version order, and upload `.deb`, `.dsc`, `.changes`, `.buildinfo`, and source artifacts. Add another Debian suite only after verifying that all dependency versions and Cargo features in `debian/control` are packaged there.

Create `release.yml` for tags matching `v*`: verify the tag’s version equals `Cargo.toml` version; verify `debian/changelog` version equals `scripts/debian-version.sh`; rerun `scripts/release-check.sh`; generate SHA-256 checksums, a CycloneDX SBOM, and GitHub artifact attestations; sign release artifacts with the repository release key; publish a GitHub release only after all checks pass. Store signing key material only in GitHub Actions secrets, never in the repository or test output.

- [ ] **Step 5: Document and execute the release-candidate procedure**

Write `docs/releasing.md` with these required candidate steps:

1. Confirm no accepted compatibility exceptions exist.
2. Run `scripts/release-check.sh` in a clean Debian environment.
3. Update `Cargo.toml` to `1.9.0-rc.1`; keep Debian changelog at `1.9.0~rc.1-1`.
4. Review `git status`, `git diff`, `git log --oneline -10`, dependency audit results, package contents, and all CI jobs.
5. Commit release metadata, create signed tag `v1.9.0-rc.1`, push the commit and tag, and verify the GitHub release artifacts/checksums/signatures.
6. Conduct the external validation window using only the published candidate; record every incompatibility as an issue with a differential regression test.
7. For stable, merge all candidate fixes, set Cargo to `1.9.0`, change changelog to `1.9.0-1`, rerun every release gate, create signed `v1.9.0`, and publish matching artifacts and release notes.

Release notes must identify the Rust rewrite, compatibility target 1.8.0, retained XML/configuration behavior, installed `pbput` coverage, upgrade path, and test methodology.

- [ ] **Step 6: Run the complete pre-candidate gate and commit release automation**

Run:

```bash
scripts/release-check.sh
git status --short
```

Expected: all checks pass and only intentionally created Debian build artifacts are untracked/ignored. Do not tag or publish during implementation; tagging is the explicit release-owner action after candidate validation.

- [ ] **Step 7: Commit CI and release procedure**

```bash
git add .github scripts docs/releasing.md README.md debian/changelog
git commit -m "ci: add pastebinit release gates"
```

## Plan Self-Review

### Spec Coverage

| Specification requirement | Plan coverage |
| --- | --- |
| Rust implementation with both installed commands | Tasks 1, 10, and 12 |
| Exact CLI, streams, exit behavior, input modes, defaults | Tasks 3, 6, 7, and 10 |
| Distribution defaults and complete configuration precedence | Tasks 4, 5, 6, and 10 |
| Form/JSON posts, timeout, redirects, regex, size/user limits | Tasks 8, 9, and 10 |
| Legacy XML support | Task 6 and Task 10 differential cases |
| Shipped pastebin definitions and gettext translations | Task 11 |
| Installed `pbput` parity | Task 12 |
| Unit, integration, and differential local-fixture tests | Tasks 2 through 12 |
| Debian-first package, same-name upgrade, correct versions | Tasks 13 and 14 |
| Candidate/stable SemVer release process and release artifacts | Task 14 |

### Placeholder Scan

The plan has no deferred implementation markers. Environment-specific values intentionally left to the release owner are limited to the actual Debian maintainer email and release signing credentials; neither can be invented safely. No task relies on a public pastebin endpoint.

### Interface Consistency

`AppError`/`AppResult` originate in Task 1. Primary-client flow is `CliOptions` (Task 3), `SiteDefinition` (Task 5), `UserPreferences` and `InlineDefaults` (Task 6), `InputDocument` (Task 7), `UploadPlan` (Task 8), then `Transport` (Task 9) and the Task 10 binary. Helper flow is `HelperEnvironment` and codecs (Task 12), then `run_pbput` (Task 12). Later task signatures use the exact earlier type names.
