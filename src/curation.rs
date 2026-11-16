use crate::{
    library::{Item, visible_game},
    progress::Progress,
};
use std::{collections::VecDeque, fs};
pub fn travel_enabled() -> bool {
    let p = crate::library::library_path()
        .parent()
        .unwrap_or(std::path::Path::new("."))
        .join("travel-picks.json");
    fs::read(p)
        .ok()
        .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
        .and_then(|v| v["travel_enabled"].as_bool())
        .unwrap_or(true)
}
pub fn mixed(items: &[Item]) -> Vec<Item> {
    let mut groups: Vec<VecDeque<Item>> = ["watch", "read", "puzzles", "atari", "listen"]
        .iter()
        .map(|kind| items.iter().filter(|i| i.kind == *kind).cloned().collect())
        .collect();
    let mut out = Vec::new();
    loop {
        let mut added = false;
        for g in &mut groups {
            if let Some(i) = g.pop_front() {
                out.push(i);
                added = true;
            }
        }
        if !added {
            break;
        }
    }
    out
}
pub fn home_items(
    items: &[Item],
    progress: &Progress,
    travel: bool,
    show_unverified: bool,
) -> Vec<Item> {
    let available: Vec<_> = items
        .iter()
        .filter(|i| visible_game(i, show_unverified))
        .cloned()
        .collect();
    let mut out = progress.continuing(&available);
    if travel {
        for i in &available {
            if i.tag == "Travel" && !out.iter().any(|x| x.id == i.id) {
                out.push(i.clone());
            }
        }
    }
    for i in mixed(&available) {
        if !out.iter().any(|x| x.id == i.id) {
            out.push(i);
        }
    }
    out
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn arbitrary_catalogue_is_interleaved_without_fixed_ids() {
        let items: Vec<_> = [
            ("x", "read"),
            ("y", "watch"),
            ("z", "watch"),
            ("q", "puzzles"),
        ]
        .iter()
        .map(|(id, k)| Item {
            id: id.to_string(),
            kind: k.to_string(),
            ..Default::default()
        })
        .collect();
        assert_eq!(
            mixed(&items)
                .iter()
                .map(|i| i.id.as_str())
                .collect::<Vec<_>>(),
            vec!["y", "x", "q", "z"]
        );
    }
}
