use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Item {
    pub id: String,
    pub title: String,
    pub subtitle: String,
    pub kind: String,
    pub tag: String,
    pub description: String,
    pub path: String,
    pub image: String,
    pub gallery: Vec<String>,
    pub source: String,
    pub meta: String,
    pub bytes: u64,
    pub seconds: u64,
    pub pages: u64,
    pub verified: bool,
    pub subtitles: bool,
    pub notes: String,
    pub folder: String,
    pub disks: Vec<String>,
    pub rom: String,
}
#[derive(Default, Deserialize, Serialize)]
#[serde(default)]
pub struct History {
    pub favorites: BTreeSet<String>,
    pub recent: Vec<String>,
}

pub fn home() -> PathBuf {
    PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
}
pub fn library_path() -> PathBuf {
    std::env::var_os("CABIN_LIBRARY")
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join(".local/share/cabin/library.json"))
}
pub fn state_dir() -> PathBuf {
    std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join(".local/state"))
        .join("cabin")
}
pub fn load() -> Result<Vec<Item>, String> {
    let p = library_path();
    let s =
        fs::read_to_string(&p).map_err(|e| format!("Cannot read library {}: {e}", p.display()))?;
    serde_json::from_str(&s).map_err(|e| format!("Library format error: {e}"))
}
pub fn save_history(h: &History) -> Result<(), String> {
    let d = state_dir();
    fs::create_dir_all(&d).map_err(|e| e.to_string())?;
    let tmp = d.join("history.json.tmp");
    fs::write(
        &tmp,
        serde_json::to_vec_pretty(h).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    fs::rename(tmp, d.join("history.json")).map_err(|e| e.to_string())
}
pub fn load_history() -> History {
    fs::read(state_dir().join("history.json"))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}
pub fn matches(item: &Item, query: &str) -> bool {
    let haystack = format!(
        "{} {} {} {}",
        item.title, item.subtitle, item.tag, item.description
    )
    .to_lowercase();
    query
        .split_whitespace()
        .all(|q| haystack.contains(&q.to_lowercase()))
}
pub fn action_label(item: &Item) -> &'static str {
    match item.kind.as_str() {
        "watch" => "Watch now",
        "listen" => "Listen now",
        "read" => {
            if item.path.ends_with(".html") {
                "Read book"
            } else {
                "Read magazine"
            }
        }
        _ => "Play now",
    }
}
pub fn command_args(item: &Item, prepare: bool) -> Result<(String, Vec<String>), String> {
    if !Path::new(&item.path).is_file() {
        return Err(format!("The local file is missing: {}", item.path));
    }
    match item.kind.as_str() {
        "watch" => Ok((
            "/usr/bin/mpv".into(),
            vec![
                "--save-position-on-quit".into(),
                "--force-window=yes".into(),
                "--hwdec=auto-safe".into(),
                "--fs".into(),
                "--af=dynaudnorm".into(),
                "--sub-auto=fuzzy".into(),
                "--sid=auto".into(),
                "--sub-visibility=yes".into(),
                "--".into(),
                item.path.clone(),
            ],
        )),
        "listen" => Ok((
            "/usr/bin/mpv".into(),
            vec![
                "--no-video".into(),
                "--force-window=yes".into(),
                "--save-position-on-quit".into(),
                "--af=dynaudnorm".into(),
                "--".into(),
                item.path.clone(),
            ],
        )),
        "read" => Ok((
            if item.path.ends_with(".html") {
                "/usr/bin/xdg-open".into()
            } else {
                "/usr/bin/evince".into()
            },
            vec![item.path.clone()],
        )),
        "puzzles" => {
            // Only known installed puzzle executables are launched; never use a shell.
            if !item.path.starts_with("/usr/bin/puzzles-") || item.path[17..].contains('/') {
                return Err("Invalid puzzle executable".into());
            }
            Ok((item.path.clone(), vec![]))
        }
        "atari" => {
            if item.folder.is_empty() || item.folder.contains('/') || item.folder == ".." {
                return Err("Invalid game folder".into());
            }
            if !Path::new(&item.rom).is_file() {
                return Err(format!("The game's ROM is missing: {}", item.rom));
            }
            let work = home().join(".local/state/atari-games").join(&item.folder);
            if prepare {
                fs::create_dir_all(&work).map_err(|e| e.to_string())?;
            }
            let mut sources = item.disks.clone();
            if item.folder == "space-crusade-voyage-beyond" {
                sources.insert(
                    0,
                    home()
                        .join("games/atari-st/space-crusade/051.MSA")
                        .to_string_lossy()
                        .into(),
                );
            }
            if sources.is_empty() {
                return Err("No disk images are listed for this game".into());
            }
            let mut copies = Vec::new();
            for disk in &sources {
                let p = Path::new(disk);
                if !p.is_file() {
                    return Err(format!("Missing disk: {disk}"));
                }
                let copy = work.join(p.file_name().ok_or("Invalid disk filename")?);
                if prepare && !copy.exists() {
                    fs::copy(p, &copy).map_err(|e| e.to_string())?;
                }
                copies.push(copy.to_string_lossy().into_owned());
            }
            let cfg = work.join("cabin.cfg");
            let native = work.join("cabin-native.keymap");
            if prepare {
                if !cfg.exists() {
                    fs::write(&cfg, "# Cabin isolated configuration\n")
                        .map_err(|e| e.to_string())?;
                }
                fs::write(&native, "").map_err(|e| e.to_string())?;
            }
            let mapping = match item.folder.as_str() {
                "laser-squad" => home().join(".local/share/laser-squad-st/arrows.keymap"),
                "sabre-team" => home().join("games/atari-st/launcher/sabre-team.keymap"),
                _ => native,
            };
            let args = vec![
                "--configfile".into(),
                cfg.to_string_lossy().into(),
                "--tos".into(),
                item.rom.clone(),
                "--machine".into(),
                "st".into(),
                "--memsize".into(),
                "1".into(),
                "--monitor".into(),
                "rgb".into(),
                "--fullscreen".into(),
                "--grab".into(),
                "--confirm-quit".into(),
                "on".into(),
                "--disk-a".into(),
                copies[0].clone(),
                "--disk-b".into(),
                copies.get(1).cloned().unwrap_or("none".into()),
                "--protect-floppy".into(),
                "off".into(),
                "--joy0".into(),
                "none".into(),
                "--joy1".into(),
                "none".into(),
                "--keymap".into(),
                mapping.to_string_lossy().into_owned(),
            ];
            Ok(("/usr/bin/hatari".into(), args))
        }
        _ => Err("Unsupported library item".into()),
    }
}
pub fn build_command(item: &Item) -> Result<Command, String> {
    let (exe, args) = command_args(item, true)?;
    if !Path::new(&exe).is_file() {
        return Err(format!("Required app is not installed: {exe}"));
    }
    let mut c = Command::new(exe);
    c.args(args);
    if item.kind == "atari" {
        c.current_dir(home().join(".local/state/atari-games").join(&item.folder));
    }
    Ok(c)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn search_matches_all_words_case_insensitively() {
        let i = Item {
            title: "Apollo 13".into(),
            tag: "Space".into(),
            ..Default::default()
        };
        assert!(matches(&i, "SPACE apollo"));
        assert!(!matches(&i, "Apollo Vietnam"));
    }
    #[test]
    fn paths_are_single_arguments_not_shell_commands() {
        let p = std::env::temp_dir().join("cabin test $(false).mkv");
        fs::write(&p, b"x").unwrap();
        let i = Item {
            kind: "watch".into(),
            path: p.to_string_lossy().into(),
            ..Default::default()
        };
        let (_, args) = command_args(&i, false).unwrap();
        assert_eq!(args.last(), Some(&i.path));
        assert_eq!(args[args.len() - 2], "--");
        for flag in [
            "--hwdec=auto-safe",
            "--fs",
            "--af=dynaudnorm",
            "--sub-auto=fuzzy",
            "--sid=auto",
            "--sub-visibility=yes",
        ] {
            assert!(args.iter().any(|a| a == flag));
        }
        fs::remove_file(p).unwrap();
    }
    #[test]
    fn missing_media_is_a_visible_error() {
        let i = Item {
            path: "/definitely/missing/cabin/video".into(),
            ..Default::default()
        };
        assert!(command_args(&i, false).unwrap_err().contains("missing"));
    }
    #[test]
    fn unknown_launcher_is_rejected() {
        let i = Item {
            kind: "puzzles".into(),
            path: "/usr/bin/sh".into(),
            ..Default::default()
        };
        assert!(command_args(&i, false).is_err());
    }
}

pub fn visible_game(item: &Item, show_unverified: bool) -> bool {
    item.kind != "atari" || item.verified || show_unverified
}
#[cfg(test)]
mod compatibility_tests {
    use super::*;
    #[test]
    fn unverified_atari_requires_opt_in() {
        let mut item = Item {
            kind: "atari".into(),
            ..Default::default()
        };
        assert!(!visible_game(&item, false));
        assert!(visible_game(&item, true));
        item.verified = true;
        assert!(visible_game(&item, false));
        item.kind = "watch".into();
        item.verified = false;
        assert!(visible_game(&item, false));
    }
}

#[cfg(test)]
mod audio_tests {
    use super::*;
    #[test]
    fn audio_uses_mpv_without_video_and_with_resume() {
        let p = std::env::temp_dir().join("cabin-audio-flags.mp3");
        fs::write(&p, b"fixture").unwrap();
        let i = Item {
            kind: "listen".into(),
            path: p.display().to_string(),
            ..Default::default()
        };
        let (exe, args) = command_args(&i, false).unwrap();
        assert_eq!(exe, "/usr/bin/mpv");
        assert!(args.contains(&"--no-video".into()));
        assert!(args.contains(&"--save-position-on-quit".into()));
        fs::remove_file(p).unwrap();
    }
}
