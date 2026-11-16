use crate::library::{Item, command_args, home};
use serde::Serialize;
use std::os::unix::fs::PermissionsExt;
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    process::Command,
};
#[derive(Default, Serialize)]
pub struct Report {
    pub items: usize,
    pub errors: Vec<String>,
    pub offline: bool,
    pub missing_players: Vec<String>,
    pub partial_files: Vec<String>,
    pub zero_byte_files: Vec<String>,
    pub hash_mismatches: Vec<String>,
    pub hashes_checked: usize,
    pub files_checked: usize,
}
fn player(path: &Path, r: &mut Report) {
    if !path.is_file() || fs::metadata(path).is_ok_and(|m| m.permissions().mode() & 0o111 == 0) {
        r.missing_players.push(path.display().to_string());
    }
}
fn walk(path: &Path, files: &mut BTreeSet<PathBuf>, errors: &mut Vec<String>) {
    if !path.exists() {
        return;
    }
    match fs::read_dir(path) {
        Ok(entries) => {
            for e in entries {
                match e {
                    Ok(e) => {
                        let p = e.path();
                        if p.is_dir() && !p.is_symlink() {
                            walk(&p, files, errors)
                        } else if p.is_file() {
                            files.insert(p);
                        }
                    }
                    Err(e) => errors.push(e.to_string()),
                }
            }
        }
        Err(e) => errors.push(format!("Cannot inspect {}: {e}", path.display())),
    }
}
pub fn check(items: &[Item]) -> Report {
    let roots = [
        "videos/flight",
        "magazines/flight",
        "books/flight",
        "music/flight",
    ]
    .map(|s| home().join(s));
    check_roots(items, &roots)
}
fn check_roots(items: &[Item], roots: &[PathBuf]) -> Report {
    let mut r = Report {
        items: items.len(),
        offline: true,
        ..Default::default()
    };
    let mut files = BTreeSet::new();
    let mut players = BTreeSet::new();
    for item in items {
        match command_args(item, false) {
            Ok((exe, _)) => {
                players.insert(PathBuf::from(exe));
            }
            Err(e) => r.errors.push(format!("{}: {e}", item.title)),
        }
        files.insert(PathBuf::from(&item.path));
        for disk in &item.disks {
            files.insert(PathBuf::from(disk));
        }
        if !item.rom.is_empty() {
            files.insert(PathBuf::from(&item.rom));
        }
    }
    for p in players {
        player(&p, &mut r);
    }
    for root in roots {
        walk(root, &mut files, &mut r.errors);
    }
    for p in &files {
        if p.extension().is_some_and(|e| e == "part") {
            r.partial_files.push(p.display().to_string());
        }
        match fs::metadata(p) {
            Ok(m) => {
                r.files_checked += 1;
                if m.len() == 0 {
                    r.zero_byte_files.push(p.display().to_string());
                }
            }
            Err(e) => r.errors.push(format!("{}: {e}", p.display())),
        }
        if p.file_name().is_some_and(|n| n == "sources.json") {
            let sources = fs::read(p).map_err(|e| e.to_string()).and_then(|b| {
                serde_json::from_slice::<serde_json::Value>(&b).map_err(|e| e.to_string())
            });
            match sources {
                Ok(sources) => {
                    if let Some(entries) = sources.as_array() {
                        for entry in entries {
                            let Some(expected) = entry["sha256"].as_str() else {
                                continue;
                            };
                            let Some(file) = entry["file"].as_str().or(entry["path"].as_str())
                            else {
                                r.errors
                                    .push(format!("Hash entry without file in {}", p.display()));
                                continue;
                            };
                            let file = if Path::new(file).is_absolute() {
                                PathBuf::from(file)
                            } else {
                                p.parent().unwrap().join(file)
                            };
                            r.hashes_checked += 1;
                            let hash = Command::new("/usr/bin/sha256sum")
                                .arg("--")
                                .arg(&file)
                                .output()
                                .ok()
                                .filter(|o| o.status.success())
                                .and_then(|o| String::from_utf8(o.stdout).ok())
                                .and_then(|s| s.split_whitespace().next().map(str::to_owned));
                            if !hash.is_some_and(|h| h.eq_ignore_ascii_case(expected)) {
                                r.hash_mismatches.push(file.display().to_string());
                            }
                        }
                    } else {
                        r.errors
                            .push(format!("Expected source list in {}", p.display()));
                    }
                }
                Err(e) => r
                    .errors
                    .push(format!("Invalid manifest {}: {e}", p.display())),
            }
        }
    }
    for (label, paths) in [
        ("Missing or non-executable player", &r.missing_players),
        ("Incomplete download", &r.partial_files),
        ("Zero-byte file", &r.zero_byte_files),
        ("SHA-256 mismatch", &r.hash_mismatches),
    ] {
        for p in paths {
            r.errors.push(format!("{label}: {p}"));
        }
    }
    r
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn incomplete_empty_and_corrupt_media_fail() {
        let root = std::env::temp_dir().join(format!("cabin-validation-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("film.mkv.part"), b"partial").unwrap();
        fs::write(root.join("empty.pdf"), b"").unwrap();
        fs::write(root.join("book.pdf"), b"abc").unwrap();
        fs::write(
            root.join("sources.json"),
            r#"[{"file":"book.pdf","sha256":"0000"}]"#,
        )
        .unwrap();
        let r = check_roots(&[], std::slice::from_ref(&root));
        assert_eq!(r.partial_files.len(), 1);
        assert_eq!(r.zero_byte_files.len(), 1);
        assert_eq!(r.hash_mismatches.len(), 1);
        assert_eq!(r.errors.len(), 3);
        fs::write(root.join("sources.json"),r#"[{"file":"book.pdf","sha256":"ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"}]"#).unwrap();
        assert!(
            check_roots(&[], std::slice::from_ref(&root))
                .hash_mismatches
                .is_empty()
        );
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn missing_executable_is_reported() {
        let mut r = Report::default();
        player(Path::new("/definitely/missing/player"), &mut r);
        assert_eq!(r.missing_players.len(), 1);
    }
}
