use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use indexmap::IndexMap;

use crate::{AppError, AppResult};

pub struct SiteDefinition {
    pub basename: String,
    pub pastebin: IndexMap<String, String>,
    pub format: IndexMap<String, String>,
    pub defaults: IndexMap<String, String>,
    pub source: PathBuf,
}

impl SiteDefinition {
    pub fn pastebin(&self, key: &str) -> Option<&str> {
        self.pastebin
            .get(&key.to_ascii_lowercase())
            .map(String::as_str)
    }
}

pub struct SiteCatalog {
    entries: IndexMap<String, SiteDefinition>,
}

impl SiteCatalog {
    pub fn get(&self, basename: &str) -> Option<&SiteDefinition> {
        self.entries.get(basename)
    }

    pub fn names_sorted(&self) -> Vec<&str> {
        let mut names: Vec<_> = self.entries.keys().map(String::as_str).collect();
        names.sort_unstable();
        names
    }
}

pub fn load_site_catalog(
    directories: &[PathBuf],
    diagnostics: &mut dyn Write,
) -> AppResult<SiteCatalog> {
    let mut entries = IndexMap::new();

    for directory in directories {
        let Ok(files) = fs::read_dir(directory) else {
            continue;
        };

        for file in files.flatten() {
            let filename = file.file_name();
            let Some(filename) = filename.to_str() else {
                continue;
            };
            if filename.starts_with('.') || !filename.ends_with(".conf") {
                continue;
            }

            let path = file.path();
            let Ok(contents) = fs::read_to_string(&path) else {
                continue;
            };
            let sections = parse_ini(&contents, &path)?;
            let Some(pastebin) = sections.get("pastebin") else {
                writeln!(diagnostics, "{}: no section [pastebin]", path.display())
                    .map_err(AppError::input)?;
                continue;
            };
            let pastebin = interpolate_section(pastebin, &path)?;
            let Some(basename) = pastebin.get("basename").cloned() else {
                writeln!(
                    diagnostics,
                    "{}: no 'basename' in [pastebin]",
                    path.display()
                )
                .map_err(AppError::input)?;
                continue;
            };

            let format = sections
                .get("format")
                .map(|section| interpolate_section(section, &path))
                .transpose()?
                .unwrap_or_default();
            let defaults = sections
                .get("defaults")
                .map(|section| interpolate_section(section, &path))
                .transpose()?
                .unwrap_or_default();
            let site = SiteDefinition {
                basename: basename.clone(),
                pastebin,
                format,
                defaults,
                source: path,
            };
            entries.insert(basename, site);
        }
    }

    Ok(SiteCatalog { entries })
}

pub fn resolve_site<'a>(
    catalog: &'a SiteCatalog,
    raw_website: &str,
) -> AppResult<(&'a SiteDefinition, String)> {
    let website = raw_website
        .split("://")
        .nth(1)
        .unwrap_or(raw_website)
        .trim_matches('/');
    let site = catalog.get(website).ok_or_else(|| {
        AppError::input(format!(
            "Unknown website, please post a bugreport to request this pastebin to be added ({website})"
        ))
    })?;
    let scheme = if site
        .pastebin("https")
        .is_some_and(|value| !value.is_empty())
    {
        "https"
    } else {
        "http"
    };

    Ok((site, format!("{scheme}://{}/", site.basename)))
}

fn parse_ini(contents: &str, path: &Path) -> AppResult<IndexMap<String, IndexMap<String, String>>> {
    let mut sections = IndexMap::new();
    let mut section_name = None;

    for (line_number, line) in contents.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            let name = line[1..line.len() - 1].to_owned();
            if sections.contains_key(&name) {
                return Err(config_error(path, line_number, "duplicate section"));
            }
            sections.insert(name.clone(), IndexMap::new());
            section_name = Some(name);
            continue;
        }

        let Some(section_name) = &section_name else {
            return Err(config_error(path, line_number, "option outside a section"));
        };
        let Some((key, value)) = line.split_once(['=', ':']) else {
            return Err(config_error(path, line_number, "invalid option"));
        };
        let key = key.trim().to_ascii_lowercase();
        let section = sections
            .get_mut(section_name)
            .expect("current section was inserted");
        if section.contains_key(&key) {
            return Err(config_error(path, line_number, "duplicate option"));
        }
        section.insert(key, value.trim().to_owned());
    }

    Ok(sections)
}

fn interpolate_section(
    section: &IndexMap<String, String>,
    path: &Path,
) -> AppResult<IndexMap<String, String>> {
    let mut resolved = IndexMap::new();
    for key in section.keys() {
        let value = interpolate_value(key, section, &mut Vec::new(), path)?;
        resolved.insert(key.clone(), value);
    }
    Ok(resolved)
}

fn interpolate_value(
    key: &str,
    section: &IndexMap<String, String>,
    resolving: &mut Vec<String>,
    path: &Path,
) -> AppResult<String> {
    if resolving.iter().any(|candidate| candidate == key) {
        return Err(AppError::input(format!(
            "{}: interpolation cycle for '{key}'",
            path.display()
        )));
    }
    let value = section
        .get(key)
        .expect("interpolation key belongs to the section");
    resolving.push(key.to_owned());

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
                        None => {
                            return Err(AppError::input(format!(
                                "{}: invalid interpolation in '{key}'",
                                path.display()
                            )));
                        }
                    }
                }
                if characters.next() != Some('s') {
                    return Err(AppError::input(format!(
                        "{}: invalid interpolation in '{key}'",
                        path.display()
                    )));
                }
                let referenced_key = referenced_key.to_ascii_lowercase();
                if !section.contains_key(&referenced_key) {
                    return Err(AppError::input(format!(
                        "{}: missing interpolation key '{referenced_key}'",
                        path.display()
                    )));
                }
                output.push_str(&interpolate_value(
                    &referenced_key,
                    section,
                    resolving,
                    path,
                )?);
            }
            _ => {
                return Err(AppError::input(format!(
                    "{}: invalid interpolation in '{key}'",
                    path.display()
                )));
            }
        }
    }

    resolving.pop();
    Ok(output)
}

fn config_error(path: &Path, line_number: usize, message: &str) -> AppError {
    AppError::input(format!("{}:{}: {message}", path.display(), line_number + 1))
}
