use std::ffi::OsString;
use std::io::{Read, Write};

use pastebinit::cli::{CliOutcome, parse_cli};
use pastebinit::config::{load_site_catalog, resolve_site};
use pastebinit::input::read_documents;
use pastebinit::platform::{
    RuntimeEnvironment, default_site_for_distro, detected_distro_id, pastebin_config_dirs,
    xml_preference_paths,
};
use pastebinit::posting::{EncodedBody, build_upload_plan};
use pastebinit::preferences::{InlineDefaults, cli_preferences, help_defaults, load_preferences};
use pastebinit::transport::{ReqwestTransport, Transport, extract_paste_url};
use pastebinit::{AppError, AppResult};

fn main() {
    let mut stdin = std::io::stdin().lock();
    let mut stdout = std::io::stdout().lock();
    let mut stderr = std::io::stderr().lock();
    if let Err(error) = execute(std::env::args_os(), &mut stdin, &mut stdout, &mut stderr) {
        let _ = writeln!(stderr, "{}", error.message());
        std::process::exit(error.exit_code());
    }
}

fn execute(
    argv: impl IntoIterator<Item = OsString>,
    stdin: &mut dyn Read,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> AppResult<()> {
    let environment = RuntimeEnvironment::from_process();
    let inline = InlineDefaults::from_process();
    let preferences = load_preferences(&xml_preference_paths(&environment))?;
    let default_site = default_site_for_distro(
        detected_distro_id(std::path::Path::new("/etc/os-release")).as_deref(),
    );
    let outcome = parse_cli(argv, &help_defaults(default_site, &inline, &preferences))?;
    match outcome {
        CliOutcome::Help(help) | CliOutcome::Version(help) => {
            write!(stdout, "{help}").map_err(AppError::input)?;
            return Ok(());
        }
        CliOutcome::Run(options) => {
            let executable = std::env::current_exe().map_err(AppError::input)?;
            let catalog =
                load_site_catalog(&pastebin_config_dirs(&environment, executable), stderr)?;
            if options.list_pastebins {
                writeln!(stdout, "Supported pastebins:").map_err(AppError::input)?;
                for name in catalog.names_sorted() {
                    writeln!(stdout, "- {name}").map_err(AppError::input)?;
                }
                return Ok(());
            }

            let cli = cli_preferences(&options);
            let website = cli
                .website
                .as_deref()
                .or(preferences.website.as_deref())
                .unwrap_or(default_site);
            let (site, _) = resolve_site(&catalog, website)?;
            let documents = read_documents(&options.files, stdin)?;
            if options.echo {
                for document in &documents {
                    writeln!(stdout, "{}", document.content).map_err(AppError::input)?;
                }
            }
            if plan_warning(site) {
                writeln!(stderr, "Warning: using target_page without paste_regexp.")
                    .map_err(AppError::input)?;
            }
            let transport = ReqwestTransport::new()?;
            for document in documents {
                let plan = build_upload_plan(site, &cli, &preferences, &inline, &document.content)?;
                if options.verbose {
                    writeln!(
                        stderr,
                        "POSTing to: {}\nParams: {}",
                        plan.url,
                        python_bytes_debug(match &plan.body {
                            EncodedBody::Form(body) | EncodedBody::Json(body) => body,
                        })
                    )
                    .map_err(AppError::input)?;
                }
                let response = transport.post(&plan).map_err(|error| {
                    AppError::input(format!("Failed to contact the server: {}", error.message()))
                })?;
                let paste_url = extract_paste_url(&plan, &response.final_url, &response.body)?;
                if options.echo {
                    writeln!(stdout, "{}", "-".repeat(paste_url.len())).map_err(AppError::input)?;
                }
                writeln!(stdout, "{paste_url}").map_err(AppError::input)?;
            }
        }
    }
    Ok(())
}

fn plan_warning(site: &pastebinit::config::SiteDefinition) -> bool {
    site.pastebin("target_page").is_some() && site.pastebin("paste_regexp").is_none()
}

fn python_bytes_debug(bytes: &[u8]) -> String {
    let quote = if bytes.contains(&b'\'') && !bytes.contains(&b'\"') {
        '"'
    } else {
        '\''
    };
    let mut rendered = String::from("b");
    rendered.push(quote);
    for &byte in bytes {
        match byte {
            b'\\' => rendered.push_str("\\\\"),
            b'\n' => rendered.push_str("\\n"),
            b'\r' => rendered.push_str("\\r"),
            b'\t' => rendered.push_str("\\t"),
            byte if byte == quote as u8 => {
                rendered.push('\\');
                rendered.push(quote);
            }
            0x20..=0x7e => rendered.push(byte as char),
            byte => {
                use std::fmt::Write as _;
                write!(rendered, "\\x{byte:02x}").expect("writing a string cannot fail");
            }
        }
    }
    rendered.push(quote);
    rendered
}
