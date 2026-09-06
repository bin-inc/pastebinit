use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use quick_xml::Reader;
use quick_xml::events::Event;

use crate::cli::{CliOptions, HelpDefaults};
use crate::{AppError, AppResult};

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
    pub fn for_user(user: &str) -> Self {
        Self {
            user: user.into(),
            title: String::new(),
            format: "text".into(),
            private: "1".into(),
            expiry: String::new(),
            username: String::new(),
            password: String::new(),
        }
    }

    pub fn from_process() -> Self {
        let user = env::var("USER").unwrap_or_else(|_| env::var("LOGNAME").unwrap_or_default());
        Self::for_user(&user)
    }

    fn value(&self, key: &str) -> &str {
        match key {
            "user" => &self.user,
            "title" => &self.title,
            "format" => &self.format,
            "private" => &self.private,
            "expiry" => &self.expiry,
            "username" => &self.username,
            "password" => &self.password,
            _ => "",
        }
    }
}

pub fn load_preferences(paths: &[PathBuf]) -> AppResult<UserPreferences> {
    let mut preferences = UserPreferences::default();

    for path in paths {
        let Ok(contents) = fs::read_to_string(path) else {
            continue;
        };
        merge_xml_preferences(&mut preferences, parse_xml_preferences(&contents, path)?);
    }

    Ok(preferences)
}

pub fn cli_preferences(options: &CliOptions) -> UserPreferences {
    UserPreferences {
        website: options.website.clone(),
        author: options.author.clone(),
        format: options.format.clone(),
        private: options.private.clone(),
        expiry: options.expiry.clone(),
        title: options.title.clone(),
        username: options.username.clone(),
        password: options.password.clone(),
    }
}

pub fn merge_preferences(mut xml: UserPreferences, cli: UserPreferences) -> UserPreferences {
    merge_xml_preferences(&mut xml, cli);
    xml
}

pub fn help_defaults(
    default_site: &str,
    inline: &InlineDefaults,
    xml: &UserPreferences,
) -> HelpDefaults {
    HelpDefaults::reference_defaults(
        xml.website.as_deref().unwrap_or(default_site),
        effective_option("user", &UserPreferences::default(), xml, inline),
    )
    .with_options(
        effective_option("format", &UserPreferences::default(), xml, inline),
        effective_option("private", &UserPreferences::default(), xml, inline),
        inline.value("expiry"),
    )
}

pub fn effective_option(
    key: &str,
    cli: &UserPreferences,
    xml: &UserPreferences,
    inline: &InlineDefaults,
) -> String {
    preference_value(cli, key)
        .or_else(|| preference_value(xml, key))
        .unwrap_or_else(|| inline.value(key))
        .into()
}

fn parse_xml_preferences(contents: &str, path: &Path) -> AppResult<UserPreferences> {
    let mut reader = Reader::from_str(contents);
    reader.config_mut().trim_text(false);
    let mut preferences = UserPreferences::default();
    let mut saw_root = false;
    let mut depth = 0;
    let mut fields = Vec::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(element)) => {
                if depth == 0 && saw_root {
                    return Err(xml_error(path));
                }
                saw_root = true;
                depth += 1;
                if let Some(key) = supported_key(element.name().as_ref())
                    && preference_value(&preferences, key).is_none()
                    && !fields.iter().any(|field: &XmlField| field.key == key)
                {
                    fields.push(XmlField {
                        key,
                        depth,
                        value: String::new(),
                    });
                }
            }
            Ok(Event::Text(text))
                if depth == 0 && !text.as_ref().iter().all(u8::is_ascii_whitespace) =>
            {
                return Err(xml_error(path));
            }
            Ok(Event::Text(text)) => {
                let value = text.unescape().map_err(|_| xml_error(path))?.into_owned();
                for field in fields.iter_mut().filter(|field| field.depth == depth) {
                    field.value.push_str(&value);
                }
            }
            Ok(Event::CData(_)) if depth == 0 => return Err(xml_error(path)),
            Ok(Event::CData(_)) => {}
            Ok(Event::Empty(element)) => {
                if depth == 0 && saw_root {
                    return Err(xml_error(path));
                }
                saw_root = true;
                if let Some(key) = supported_key(element.name().as_ref())
                    && !fields.iter().any(|field: &XmlField| field.key == key)
                {
                    set_xml_value(&mut preferences, key, String::new());
                }
            }
            Ok(Event::End(_)) => {
                for index in (0..fields.len()).rev() {
                    if fields[index].depth == depth {
                        let field = fields.remove(index);
                        set_xml_value(&mut preferences, field.key, field.value);
                    }
                }
                depth = depth.checked_sub(1).ok_or_else(|| xml_error(path))?;
            }
            Ok(Event::Eof) if saw_root && depth == 0 => break,
            Ok(Event::Eof) => return Err(xml_error(path)),
            Ok(_) => {}
            Err(_) => return Err(xml_error(path)),
        }
    }

    Ok(preferences)
}

struct XmlField {
    key: &'static str,
    depth: usize,
    value: String,
}

fn supported_key(name: &[u8]) -> Option<&'static str> {
    match name {
        b"pastebin" => Some("website"),
        b"author" => Some("author"),
        b"format" => Some("format"),
        b"private" => Some("private"),
        b"expiry" => Some("expiry"),
        _ => None,
    }
}

fn merge_xml_preferences(target: &mut UserPreferences, source: UserPreferences) {
    if source.website.is_some() {
        target.website = source.website;
    }
    if source.author.is_some() {
        target.author = source.author;
    }
    if source.format.is_some() {
        target.format = source.format;
    }
    if source.private.is_some() {
        target.private = source.private;
    }
    if source.expiry.is_some() {
        target.expiry = source.expiry;
    }
    if source.title.is_some() {
        target.title = source.title;
    }
    if source.username.is_some() {
        target.username = source.username;
    }
    if source.password.is_some() {
        target.password = source.password;
    }
}

fn preference_value<'a>(preferences: &'a UserPreferences, key: &str) -> Option<&'a str> {
    match key {
        "website" => preferences.website.as_deref(),
        "user" => preferences.author.as_deref(),
        "format" => preferences.format.as_deref(),
        "private" => preferences.private.as_deref(),
        "expiry" => preferences.expiry.as_deref(),
        "title" => preferences.title.as_deref(),
        "username" => preferences.username.as_deref(),
        "password" => preferences.password.as_deref(),
        _ => None,
    }
}

fn set_xml_value(preferences: &mut UserPreferences, key: &str, value: String) {
    match key {
        "website" if preferences.website.is_none() => preferences.website = Some(value),
        "author" if preferences.author.is_none() => preferences.author = Some(value),
        "format" if preferences.format.is_none() => preferences.format = Some(value),
        "private" if preferences.private.is_none() => preferences.private = Some(value),
        "expiry" if preferences.expiry.is_none() => preferences.expiry = Some(value),
        _ => {}
    }
}

fn xml_error(_path: &Path) -> AppError {
    AppError::input(
        "Error parsing configuration file!\nPlease ensure that your configuration file looks similar to the following:\n    <pastebinit>\n        <pastebin>paste.debian.net</pastebin>\n        <author>A pastebinit user</author>\n        <format>text</format>\n        <private>1</private>\n        <expiry></expiry>\n    </pastebinit>\n    ",
    )
}
