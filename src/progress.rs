use crate::library::{Item, home};
use std::{
    collections::HashMap,
    fs,
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
    time::SystemTime,
};
#[derive(Clone, Default)]
pub struct Position {
    pub seconds: f64,
    pub modified: Option<SystemTime>,
}
pub struct Progress {
    paths: HashMap<String, Vec<PathBuf>>,
    pub positions: HashMap<String, Position>,
}
pub fn parse_start(text: &str) -> Option<f64> {
    text.lines()
        .find_map(|line| line.strip_prefix("start="))
        .and_then(|v| v.trim().parse::<f64>().ok())
        .filter(|s| s.is_finite() && *s >= 0.)
}
fn key(path: &str) -> Option<String> {
    let mut c = Command::new("/usr/bin/md5sum")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .ok()?;
    c.stdin.take()?.write_all(path.as_bytes()).ok()?;
    let o = c.wait_with_output().ok()?;
    if !o.status.success() {
        return None;
    }
    String::from_utf8(o.stdout)
        .ok()?
        .split_whitespace()
        .next()
        .map(str::to_uppercase)
}
impl Progress {
    pub fn new(items: &[Item]) -> Self {
        let state = std::env::var_os("XDG_STATE_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home().join(".local/state"));
        let config = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home().join(".config"));
        let mut roots = vec![
            state.join("mpv/watch_later"),
            config.join("mpv/watch_later"),
        ];
        if let Some(p) = std::env::var_os("MPV_HOME") {
            roots.push(PathBuf::from(p).join("watch_later"));
        }
        let paths = items
            .iter()
            .filter(|i| i.kind == "watch" || i.kind == "listen")
            .filter_map(|i| {
                key(&i.path).map(|k| (i.id.clone(), roots.iter().map(|r| r.join(&k)).collect()))
            })
            .collect();
        let mut this = Self {
            paths,
            positions: HashMap::new(),
        };
        this.refresh();
        this
    }
    pub fn refresh(&mut self) {
        self.positions.clear();
        for (id, paths) in &self.paths {
            let newest = paths
                .iter()
                .filter_map(|p| {
                    fs::read_to_string(p)
                        .ok()
                        .and_then(|s| parse_start(&s))
                        .map(|seconds| Position {
                            seconds,
                            modified: fs::metadata(p).ok().and_then(|m| m.modified().ok()),
                        })
                })
                .max_by_key(|p| p.modified);
            if let Some(p) = newest {
                self.positions.insert(id.clone(), p);
            }
        }
    }
    pub fn seconds(&self, item: &Item) -> f64 {
        self.positions
            .get(&item.id)
            .map_or(0., |p| p.seconds)
            .min(item.seconds as f64)
    }
    pub fn unfinished(&self, item: &Item) -> bool {
        let p = self.seconds(item);
        p >= 5. && item.seconds as f64 - p > 30. && p / (item.seconds.max(1) as f64) < 0.98
    }
    pub fn continuing(&self, items: &[Item]) -> Vec<Item> {
        let mut v: Vec<_> = items
            .iter()
            .filter(|i| self.unfinished(i))
            .cloned()
            .collect();
        v.sort_by_key(|i| std::cmp::Reverse(self.positions.get(&i.id).and_then(|p| p.modified)));
        v
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_real_numeric_watch_later() {
        assert_eq!(parse_start("start=123.456\n"), Some(123.456));
        for s in ["start=NaN", "start=-1", "volume=20", ""] {
            assert_eq!(parse_start(s), None);
        }
    }
    #[test]
    fn excludes_finished_and_trivial_progress() {
        let item = Item {
            id: "v".into(),
            seconds: 1000,
            ..Default::default()
        };
        let mut p = Progress {
            paths: HashMap::new(),
            positions: HashMap::new(),
        };
        for (value, expected) in [(1., false), (100., true), (985., false), (2000., false)] {
            p.positions.insert(
                "v".into(),
                Position {
                    seconds: value,
                    ..Default::default()
                },
            );
            assert_eq!(p.unfinished(&item), expected);
        }
    }
    #[test]
    fn mpv_key_is_uppercase_md5_of_path() {
        assert_eq!(key("abc").unwrap(), "900150983CD24FB0D6963F7D28E17F72");
    }
}
