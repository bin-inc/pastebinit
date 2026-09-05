use std::ffi::OsString;

use clap::{Arg, ArgAction, Command};

use crate::{AppError, AppResult, VERSION};

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

impl HelpDefaults {
    pub fn reference_defaults(website: impl Into<String>, author: impl Into<String>) -> Self {
        Self {
            website: website.into(),
            author: author.into(),
            format: "text".into(),
            private: "1".into(),
            expiry: String::new(),
        }
    }

    pub fn with_options(
        mut self,
        format: impl Into<String>,
        private: impl Into<String>,
        expiry: impl Into<String>,
    ) -> Self {
        self.format = format.into();
        self.private = private.into();
        self.expiry = expiry.into();
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CliOutcome {
    Run(CliOptions),
    Help(String),
    Version(String),
}

pub fn parse_cli<I, T>(args: I, defaults: &HelpDefaults) -> AppResult<CliOutcome>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let args: Vec<OsString> = args.into_iter().map(Into::into).collect();
    let matches = command()
        .try_get_matches_from(args)
        .map_err(|error| AppError::usage(argparse_error(&error.to_string())))?;
    if matches.get_flag("help") {
        return Ok(CliOutcome::Help(render_help(defaults)));
    }
    if matches.get_flag("version") {
        return Ok(CliOutcome::Version(format!("pastebinit v{VERSION}\n")));
    }
    let mut files = values(&matches, "input");
    files.extend(values(&matches, "files"));

    Ok(CliOutcome::Run(CliOptions {
        files,
        website: value(&matches, "website"),
        list_pastebins: matches.get_flag("list-pastebins"),
        echo: matches.get_flag("echo"),
        verbose: matches.get_flag("verbose"),
        author: value(&matches, "author"),
        title: value(&matches, "title"),
        format: value(&matches, "format"),
        private: value(&matches, "private"),
        expiry: value(&matches, "expiry"),
        username: value(&matches, "username"),
        password: value(&matches, "password"),
    }))
}

fn argparse_error(error: &str) -> String {
    let detail = if let Some(argument) = error
        .strip_prefix("error: unexpected argument '")
        .and_then(|value| value.split_once("' found"))
        .map(|(argument, _)| argument)
    {
        format!("unrecognized arguments: {argument}")
    } else if let Some(argument) = error
        .strip_prefix("error: a value is required for '")
        .and_then(|value| value.split_once("' but none was supplied"))
        .map(|(argument, _)| argument)
    {
        if argument == "-i <FILE>..." {
            "argument -i: expected at least one argument".into()
        } else {
            let argument = argument.split_whitespace().next().unwrap_or(argument);
            format!("argument {argument}: expected one argument")
        }
    } else {
        return error.to_owned();
    };
    format!("usage: pastebinit [OPTION...] [FILE...]\npastebinit: error: {detail}")
}

pub fn render_help(defaults: &HelpDefaults) -> String {
    format!(
        concat!(
            "usage: pastebinit [OPTION...] [FILE...]\n\n",
            "Reads on stdin for input or takes a list of files as parameters\n\n",
            "General arguments:\n",
            "  -b <pastebin>         (default is '{}')\n",
            "  -i FILE [FILE ...]    One or more files to read and paste\n",
            "  -l                    List all supported pastebins\n",
            "  -E                    Print the content to stdout too\n",
            "  -h                    Print this help screen\n",
            "  -v                    Print the version number\n",
            "  -V                    Print verbose output to stderr\n\n",
            "Optional arguments (not supported by all pastebins):\n",
            "  -a <author>           (default is '{}')\n",
            "  -t <title of paste>   (default is '')\n",
            "  -f <format of paste>  (default is '{}')\n",
            "  -P <private level>    (default is '{}')\n",
            "  -e <expiry of paste>  (default is '{}')\n",
            "  -u <username>         (default is '')\n",
            "  -p <password>         (default is '')\n",
        ),
        defaults.website, defaults.author, defaults.format, defaults.private, defaults.expiry,
    )
}

fn command() -> Command {
    Command::new("pastebinit")
        .disable_help_flag(true)
        .disable_version_flag(true)
        .arg(Arg::new("help").short('h').action(ArgAction::SetTrue))
        .arg(Arg::new("version").short('v').action(ArgAction::SetTrue))
        .arg(Arg::new("website").short('b').action(ArgAction::Set))
        .arg(
            Arg::new("input")
                .short('i')
                .value_name("FILE")
                .num_args(1..)
                .action(ArgAction::Append),
        )
        .arg(
            Arg::new("list-pastebins")
                .short('l')
                .action(ArgAction::SetTrue),
        )
        .arg(Arg::new("echo").short('E').action(ArgAction::SetTrue))
        .arg(Arg::new("verbose").short('V').action(ArgAction::SetTrue))
        .arg(Arg::new("author").short('a').action(ArgAction::Set))
        .arg(Arg::new("title").short('t').action(ArgAction::Set))
        .arg(Arg::new("format").short('f').action(ArgAction::Set))
        .arg(Arg::new("private").short('P').action(ArgAction::Set))
        .arg(Arg::new("expiry").short('e').action(ArgAction::Set))
        .arg(Arg::new("username").short('u').action(ArgAction::Set))
        .arg(Arg::new("password").short('p').action(ArgAction::Set))
        .arg(
            Arg::new("files")
                .value_name("FILE")
                .num_args(0..)
                .action(ArgAction::Append),
        )
}

fn value(matches: &clap::ArgMatches, name: &str) -> Option<String> {
    matches.get_one::<String>(name).cloned()
}

fn values(matches: &clap::ArgMatches, name: &str) -> Vec<String> {
    matches
        .get_many::<String>(name)
        .map(|values| values.cloned().collect())
        .unwrap_or_default()
}
