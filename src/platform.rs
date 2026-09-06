use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

pub struct RuntimeEnvironment {
    pub home: PathBuf,
    pub xdg_data_dirs: String,
    pub xdg_config_dirs: String,
    pub xdg_config_home: PathBuf,
}

impl RuntimeEnvironment {
    pub fn from_process() -> Self {
        let home = env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("~"));
        let xdg_config_home = env::var_os("XDG_CONFIG_HOME")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".config"));

        Self {
            home,
            xdg_data_dirs: env::var("XDG_DATA_DIRS").unwrap_or_default(),
            xdg_config_dirs: env::var("XDG_CONFIG_DIRS").unwrap_or_default(),
            xdg_config_home,
        }
    }

    pub fn for_test(home: &str, data_dirs: &str, config_dirs: &str, config_home: &str) -> Self {
        let home = PathBuf::from(home);
        let xdg_config_home = if config_home.is_empty() {
            home.join(".config")
        } else {
            PathBuf::from(config_home)
        };

        Self {
            home,
            xdg_data_dirs: data_dirs.to_owned(),
            xdg_config_dirs: config_dirs.to_owned(),
            xdg_config_home,
        }
    }
}

pub fn default_site_for_distro(distro_id: Option<&str>) -> &'static str {
    match distro_id {
        Some("debian" | "raspbian") => "paste.debian.net",
        Some("fedora" | "centos" | "rhel" | "rocky") => "paste.centos.org",
        Some(id) if id.contains("suse") || id == "sles" => "paste.opensuse.org",
        _ => "bpa.st",
    }
}

pub fn detected_distro_id(os_release_path: &Path) -> Option<String> {
    let contents = fs::read_to_string(os_release_path).ok()?;

    contents.lines().find_map(|line| {
        let value = line.strip_prefix("ID=")?;
        Some(value.trim_matches('"').to_owned())
    })
}

pub fn pastebin_config_dirs(
    env: &RuntimeEnvironment,
    executable: impl AsRef<Path>,
) -> Vec<PathBuf> {
    let mut bases = vec![
        PathBuf::from("/usr/share"),
        PathBuf::from("/usr/local/share"),
    ];
    bases.extend(reversed_xdg_paths(&env.xdg_data_dirs));
    bases.extend(reversed_xdg_paths(&env.xdg_config_dirs));
    bases.extend([
        PathBuf::from("/etc"),
        PathBuf::from("/usr/local/etc"),
        env.xdg_config_home.clone(),
        env.home.join(".pastebin.d"),
        executable_parent(executable.as_ref()),
    ]);

    suffixed_unique_paths(bases, "pastebin.d")
}

pub fn xml_preference_paths(env: &RuntimeEnvironment) -> Vec<PathBuf> {
    let mut bases: Vec<_> = reversed_xdg_paths(&env.xdg_config_dirs).collect();
    bases.extend([
        PathBuf::from("/etc"),
        PathBuf::from("/usr/local/etc"),
        env.xdg_config_home.clone(),
        env.home.join(".pastebinit.xml"),
    ]);

    suffixed_unique_paths(bases, "pastebinit.xml")
}

fn reversed_xdg_paths(value: &str) -> impl Iterator<Item = PathBuf> + '_ {
    value.split(':').rev().map(PathBuf::from)
}

fn executable_parent(executable: &Path) -> PathBuf {
    let parent = executable.parent().unwrap_or_else(|| Path::new("."));
    let absolute_parent = if parent.is_absolute() {
        parent.to_path_buf()
    } else {
        env::current_dir()
            .map(|current_dir| current_dir.join(parent))
            .unwrap_or_else(|_| parent.to_path_buf())
    };

    fs::canonicalize(&absolute_parent).unwrap_or(absolute_parent)
}

fn suffixed_unique_paths(bases: impl IntoIterator<Item = PathBuf>, suffix: &str) -> Vec<PathBuf> {
    let mut seen = HashSet::new();

    bases
        .into_iter()
        .map(|base| append_suffix(base, suffix))
        .filter(|path| seen.insert(path.clone()))
        .collect()
}

fn append_suffix(base: PathBuf, suffix: &str) -> PathBuf {
    let path = base.to_string_lossy();
    let trimmed = path.trim_end_matches('/');

    if trimmed.ends_with(suffix) {
        PathBuf::from(trimmed)
    } else {
        PathBuf::from(trimmed).join(suffix)
    }
}
