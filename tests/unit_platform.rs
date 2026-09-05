use std::path::{Path, PathBuf};

use pastebinit::platform::{
    RuntimeEnvironment, default_site_for_distro, detected_distro_id, pastebin_config_dirs,
    xml_preference_paths,
};

#[test]
fn distro_defaults_match_pastebinit_1_8_0() {
    assert_eq!(default_site_for_distro(Some("debian")), "paste.debian.net");
    assert_eq!(
        default_site_for_distro(Some("raspbian")),
        "paste.debian.net"
    );
    assert_eq!(default_site_for_distro(Some("fedora")), "paste.centos.org");
    assert_eq!(default_site_for_distro(Some("centos")), "paste.centos.org");
    assert_eq!(default_site_for_distro(Some("rhel")), "paste.centos.org");
    assert_eq!(default_site_for_distro(Some("rocky")), "paste.centos.org");
    assert_eq!(
        default_site_for_distro(Some("opensuse-tumbleweed")),
        "paste.opensuse.org"
    );
    assert_eq!(default_site_for_distro(Some("sles")), "paste.opensuse.org");
    assert_eq!(default_site_for_distro(Some("ubuntu")), "bpa.st");
    assert_eq!(default_site_for_distro(None), "bpa.st");
}

#[test]
fn config_directories_follow_reference_precedence() {
    let env = RuntimeEnvironment::for_test(
        "/home/alice",
        "/data/a:/data/b",
        "/cfg/a:/cfg/b",
        "/cfg/home",
    );

    let dirs = pastebin_config_dirs(&env, "/opt/pastebinit/pastebinit");

    assert_eq!(dirs[0], Path::new("/usr/share/pastebin.d"));
    assert_eq!(
        dirs.last(),
        Some(&Path::new("/opt/pastebinit/pastebin.d").to_path_buf())
    );
    assert!(
        dirs.iter()
            .position(|path| path == Path::new("/data/b/pastebin.d"))
            < dirs
                .iter()
                .position(|path| path == Path::new("/data/a/pastebin.d"))
    );
    assert!(
        dirs.iter()
            .position(|path| path == Path::new("/cfg/b/pastebin.d"))
            < dirs
                .iter()
                .position(|path| path == Path::new("/cfg/a/pastebin.d"))
    );
}

#[test]
fn config_directories_keep_empty_xdg_entries_as_relative_paths() {
    let env = RuntimeEnvironment::for_test("/home/alice", "", "/cfg", "/cfg/home");

    let dirs = pastebin_config_dirs(&env, "/opt/pastebinit/pastebinit");

    assert!(dirs.contains(&Path::new("pastebin.d").to_path_buf()));
}

#[test]
fn config_directories_do_not_double_append_pastebin_directory_suffix() {
    let env = RuntimeEnvironment::for_test("/home/alice", "/data/pastebin.d", "/cfg", "/cfg/home");

    let dirs = pastebin_config_dirs(&env, "/opt/pastebinit/pastebinit");

    assert!(dirs.contains(&Path::new("/data/pastebin.d").to_path_buf()));
    assert!(!dirs.contains(&Path::new("/data/pastebin.d/pastebin.d").to_path_buf()));
}

#[test]
fn config_directories_deduplicate_final_paths_without_reordering() {
    let env =
        RuntimeEnvironment::for_test("/home/alice", "/usr/share:/data", "/data:/etc", "/data");

    let dirs = pastebin_config_dirs(&env, "/data/pastebinit");

    assert_eq!(
        dirs,
        vec![
            Path::new("/usr/share/pastebin.d").to_path_buf(),
            Path::new("/usr/local/share/pastebin.d").to_path_buf(),
            Path::new("/data/pastebin.d").to_path_buf(),
            Path::new("/etc/pastebin.d").to_path_buf(),
            Path::new("/usr/local/etc/pastebin.d").to_path_buf(),
            Path::new("/home/alice/.pastebin.d").to_path_buf(),
        ]
    );
}

#[test]
fn test_environment_uses_home_config_when_xdg_config_home_is_unset() {
    let env = RuntimeEnvironment::for_test("/home/alice", "/data", "/cfg", "");

    assert_eq!(env.xdg_config_home, Path::new("/home/alice/.config"));
}

#[test]
fn xml_preference_paths_follow_reference_precedence() {
    let env = RuntimeEnvironment::for_test("/home/alice", "/data", "/cfg/a:/cfg/b", "/cfg/home");

    assert_eq!(
        xml_preference_paths(&env),
        vec![
            Path::new("/cfg/b/pastebinit.xml").to_path_buf(),
            Path::new("/cfg/a/pastebinit.xml").to_path_buf(),
            Path::new("/etc/pastebinit.xml").to_path_buf(),
            Path::new("/usr/local/etc/pastebinit.xml").to_path_buf(),
            Path::new("/cfg/home/pastebinit.xml").to_path_buf(),
            Path::new("/home/alice/.pastebinit.xml").to_path_buf(),
        ]
    );
}

#[test]
fn detected_distro_id_reads_the_os_release_id_field() {
    let file = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(file.path(), "NAME=Example\nID=\"opensuse-tumbleweed\"\n").unwrap();

    assert_eq!(
        detected_distro_id(file.path()).as_deref(),
        Some("opensuse-tumbleweed")
    );
}

#[test]
fn detected_distro_id_accepts_unquoted_id_values() {
    let file = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(file.path(), "NAME=Example\nID=fedora\n").unwrap();

    assert_eq!(detected_distro_id(file.path()).as_deref(), Some("fedora"));
}

#[test]
fn detected_distro_id_returns_none_when_os_release_has_no_id() {
    let file = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(file.path(), "NAME=Example\nVERSION_ID=1\n").unwrap();

    assert_eq!(detected_distro_id(file.path()), None);
}

#[test]
fn detected_distro_id_returns_none_when_os_release_is_missing() {
    assert_eq!(
        detected_distro_id(Path::new("/definitely/missing/os-release")),
        None
    );
}

#[test]
fn nonexistent_executable_under_symlinked_parent_uses_resolved_parent() {
    let root = tempfile::tempdir().unwrap();
    let resolved_parent = root.path().join("resolved");
    std::fs::create_dir(&resolved_parent).unwrap();
    let linked_parent = root.path().join("linked");
    std::os::unix::fs::symlink(&resolved_parent, &linked_parent).unwrap();
    let config_dirs = resolved_parent.to_string_lossy();
    let env = RuntimeEnvironment::for_test("/home/alice", "", &config_dirs, "/cfg/home");

    let dirs = pastebin_config_dirs(&env, linked_parent.join("pastebinit"));

    let resolved_config = resolved_parent.join("pastebin.d");
    assert_eq!(
        dirs.iter().filter(|path| *path == &resolved_config).count(),
        1
    );
    assert!(!dirs.contains(&linked_parent.join("pastebin.d")));
}

#[test]
fn nonexistent_relative_executable_uses_the_current_directory() {
    let env = RuntimeEnvironment::for_test("/home/alice", "/data", "/cfg", "/cfg/home");

    let dirs = pastebin_config_dirs(&env, "pastebinit");

    let current_config = std::env::current_dir().unwrap().join("pastebin.d");
    assert!(dirs.contains(&current_config));
    assert!(!dirs.contains(&PathBuf::from("pastebin.d")));
}
