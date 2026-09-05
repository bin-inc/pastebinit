use indexmap::IndexMap;

use crate::config::SiteDefinition;
use crate::preferences::{InlineDefaults, UserPreferences, effective_option};
use crate::{AppError, AppResult};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EncodedBody {
    Form(Vec<u8>),
    Json(Vec<u8>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
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
) -> AppResult<UploadPlan> {
    if let Some(limit) = site.pastebin("sizelimit") {
        let limit = limit
            .parse::<usize>()
            .map_err(|_| AppError::input("The pastebin's size limit is not a valid integer."))?;
        if content.chars().count() > limit {
            return Err(AppError::input(
                "The content you are trying to send exceeds the pastebin's size limit.",
            ));
        }
    }

    let mut overrides = IndexMap::new();
    for key in [
        "user", "title", "format", "private", "expiry", "username", "password",
    ] {
        if let Some(value) = preference_value(cli, key).or_else(|| preference_value(xml, key)) {
            overrides.insert(key.to_owned(), value.to_owned());
        }
    }

    if let Some(limit) = site.pastebin("user_length") {
        let limit = limit
            .parse::<isize>()
            .map_err(|_| AppError::input("The pastebin's user length is not a valid integer."))?;
        let user = parameter_value("user", site, &overrides, cli, xml, inline)?;
        if user.chars().count() as isize > limit {
            overrides.insert("user".into(), truncate_python_user(&user, limit));
        }
    }

    let mut parameters = IndexMap::new();
    for (key, name) in &site.format {
        let value = if key == "content" {
            content.to_owned()
        } else {
            parameter_value(key, site, &overrides, cli, xml, inline)?
        };
        parameters.insert(name.clone(), value);
    }

    let scheme = if site
        .pastebin("https")
        .is_some_and(|value| !value.is_empty())
    {
        "https"
    } else {
        "http"
    };
    let base_url = format!("{scheme}://{}/", site.basename);
    let url = match site.pastebin("post_page") {
        Some(page) => format!("{}{}", base_url, page.trim_start_matches('/')),
        None => base_url.clone(),
    };
    let target_url = site
        .pastebin("target_page")
        .map(|page| format!("{}{}", base_url, page.trim_start_matches('/')));

    let (body, content_type) = if site.pastebin("post_format") == Some("json") {
        (
            EncodedBody::Json(python_json_object(&parameters)),
            Some("text/json".into()),
        )
    } else {
        (EncodedBody::Form(python_form_urlencode(&parameters)), None)
    };

    Ok(UploadPlan {
        url,
        base_url,
        body,
        content_type,
        response_pattern: site.pastebin("paste_regexp").map(str::to_owned),
        target_url,
    })
}

pub fn python_form_urlencode(parameters: &IndexMap<String, String>) -> Vec<u8> {
    let mut encoded = Vec::new();
    for (index, (key, value)) in parameters.iter().enumerate() {
        if index != 0 {
            encoded.push(b'&');
        }
        form_component(&mut encoded, key);
        encoded.push(b'=');
        form_component(&mut encoded, value);
    }
    encoded
}

pub fn python_json_object(parameters: &IndexMap<String, String>) -> Vec<u8> {
    let mut encoded = Vec::from(b"{".as_slice());
    for (index, (key, value)) in parameters.iter().enumerate() {
        if index != 0 {
            encoded.extend_from_slice(b", ");
        }
        json_string(&mut encoded, key);
        encoded.extend_from_slice(b": ");
        json_string(&mut encoded, value);
    }
    encoded.push(b'}');
    encoded
}

fn parameter_value(
    key: &str,
    site: &SiteDefinition,
    overrides: &IndexMap<String, String>,
    cli: &UserPreferences,
    xml: &UserPreferences,
    inline: &InlineDefaults,
) -> AppResult<String> {
    if let Some(value) = overrides.get(key) {
        return Ok(value.clone());
    }
    if site.defaults.contains_key(key) {
        return interpolate_default(key, site, overrides, &mut Vec::new());
    }
    Ok(effective_option(key, cli, xml, inline))
}

fn interpolate_default(
    key: &str,
    site: &SiteDefinition,
    overrides: &IndexMap<String, String>,
    resolving: &mut Vec<String>,
) -> AppResult<String> {
    if let Some(value) = overrides.get(key) {
        return Ok(value.clone());
    }
    if resolving.iter().any(|candidate| candidate == key) {
        return Err(AppError::input(format!(
            "{}: interpolation cycle for '{key}'",
            site.source.display()
        )));
    }
    let Some(value) = site.defaults.get(key) else {
        return Err(AppError::input(format!(
            "{}: missing interpolation key '{key}'",
            site.source.display()
        )));
    };
    resolving.push(key.to_owned());

    let result = interpolate(
        value,
        |referenced_key| interpolate_default(referenced_key, site, overrides, resolving),
        key,
        site,
    );
    resolving.pop();
    result
}

fn interpolate(
    value: &str,
    mut resolve: impl FnMut(&str) -> AppResult<String>,
    key: &str,
    site: &SiteDefinition,
) -> AppResult<String> {
    let mut output = String::new();
    let mut characters = value.chars();
    while let Some(character) = characters.next() {
        if character != '%' {
            output.push(character);
            continue;
        }
        match characters.next() {
            Some('%') => output.push('%'),
            Some('(') => {
                let mut referenced_key = String::new();
                loop {
                    match characters.next() {
                        Some(')') => break,
                        Some(character) => referenced_key.push(character),
                        None => return Err(interpolation_error(key, site)),
                    }
                }
                if characters.next() != Some('s') {
                    return Err(interpolation_error(key, site));
                }
                output.push_str(&resolve(&referenced_key.to_ascii_lowercase())?);
            }
            _ => return Err(interpolation_error(key, site)),
        }
    }
    Ok(output)
}

fn interpolation_error(key: &str, site: &SiteDefinition) -> AppError {
    AppError::input(format!(
        "{}: invalid interpolation in '{key}'",
        site.source.display()
    ))
}

fn truncate_python_user(user: &str, limit: isize) -> String {
    let characters: Vec<char> = user.chars().collect();
    let end = limit - 1;
    let end = if end < 0 {
        characters.len().saturating_sub(end.unsigned_abs())
    } else {
        (end as usize).min(characters.len())
    };
    let mut truncated: String = characters[..end].iter().collect();
    truncated.push('~');
    truncated
}

fn preference_value<'a>(preferences: &'a UserPreferences, key: &str) -> Option<&'a str> {
    match key {
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

fn form_component(output: &mut Vec<u8>, value: &str) {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'_' | b'.' | b'-' | b'~' => {
                output.push(byte)
            }
            b' ' => output.push(b'+'),
            _ => {
                output.extend_from_slice(&[
                    b'%',
                    HEX[(byte >> 4) as usize],
                    HEX[(byte & 15) as usize],
                ]);
            }
        }
    }
}

fn json_string(output: &mut Vec<u8>, value: &str) {
    output.push(b'\"');
    for character in value.chars() {
        match character {
            '\"' => output.extend_from_slice(b"\\\""),
            '\\' => output.extend_from_slice(b"\\\\"),
            '\u{08}' => output.extend_from_slice(b"\\b"),
            '\u{0C}' => output.extend_from_slice(b"\\f"),
            '\n' => output.extend_from_slice(b"\\n"),
            '\r' => output.extend_from_slice(b"\\r"),
            '\t' => output.extend_from_slice(b"\\t"),
            character if character <= '\u{1F}' => json_escape_u16(output, character as u16),
            character if character.is_ascii() => output.push(character as u8),
            character if (character as u32) <= 0xFFFF => json_escape_u16(output, character as u16),
            character => {
                let scalar = character as u32 - 0x1_0000;
                json_escape_u16(output, (0xD800 + (scalar >> 10)) as u16);
                json_escape_u16(output, (0xDC00 + (scalar & 0x3FF)) as u16);
            }
        }
    }
    output.push(b'\"');
}

fn json_escape_u16(output: &mut Vec<u8>, value: u16) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    output.extend_from_slice(&[
        b'\\',
        b'u',
        HEX[((value >> 12) & 15) as usize],
        HEX[((value >> 8) & 15) as usize],
        HEX[((value >> 4) & 15) as usize],
        HEX[(value & 15) as usize],
    ]);
}
