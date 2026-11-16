use crate::library::Item;
use std::{collections::HashMap, fs, path::Path};
#[derive(Clone)]
pub struct Slot {
    pub item: Item,
    pub minutes: u32,
    pub estimated: bool,
}
pub fn lineup(items: &[Item], budget: u32, remaining: &HashMap<String, u64>) -> Vec<Slot> {
    let mut left = budget;
    let mut out = Vec::new();
    let has_read = items.iter().any(|i| i.kind == "read");
    let has_game = items
        .iter()
        .any(|i| i.kind == "puzzles" || (i.kind == "atari" && i.verified));
    let reserve = if budget >= 45 {
        (if has_read { 20 } else { 0 }) + (if has_game { 15 } else { 0 })
    } else {
        0
    };
    // Films only: audio is for listening alongside, and a book's chapters would crowd the plan.
    for i in items.iter().filter(|i| i.kind == "watch") {
        let seconds = *remaining.get(&i.id).unwrap_or(&i.seconds);
        let duration = seconds.div_ceil(60) as u32;
        if duration > 0 && duration <= left.saturating_sub(reserve) {
            out.push(Slot {
                item: i.clone(),
                minutes: duration,
                estimated: false,
            });
            left -= duration;
        }
    }
    for (kind, duration) in [("read", 20), ("puzzles", 15)] {
        if left < 5 {
            break;
        }
        let found = items
            .iter()
            .find(|i| i.kind == kind || (kind == "puzzles" && i.kind == "atari" && i.verified));
        if let Some(i) = found {
            let m = duration.min(left);
            out.push(Slot {
                item: i.clone(),
                minutes: m,
                estimated: true,
            });
            left -= m;
        }
    }
    out
}
/// A line-up per flight for a trip with stops. Nothing is planned twice, and
/// no film is split across a stop.
pub fn trip(items: &[Item], legs: &[u32], remaining: &HashMap<String, u64>) -> Vec<Vec<Slot>> {
    let mut left: Vec<Item> = items.to_vec();
    legs.iter()
        .map(|&budget| {
            let plan = lineup(&left, budget, remaining);
            left.retain(|i| !plan.iter().any(|s| s.item.id == i.id));
            plan
        })
        .collect()
}
#[derive(Default)]
pub struct Battery {
    pub percent: Option<u8>,
    pub status: String,
    pub hours: Option<f64>,
}
impl Battery {
    pub fn read() -> Self {
        Self::from_dir(Path::new("/sys/class/power_supply/BAT0"))
    }
    fn from_dir(root: &Path) -> Self {
        let number = |name: &str| {
            fs::read_to_string(root.join(name))
                .ok()
                .and_then(|s| s.trim().parse::<f64>().ok())
        };
        let status = fs::read_to_string(root.join("status"))
            .unwrap_or_else(|_| "Unavailable".into())
            .trim()
            .to_owned();
        let pair = number("energy_now")
            .zip(number("power_now"))
            .or_else(|| number("charge_now").zip(number("current_now")));
        let hours = if status == "Discharging" {
            pair.and_then(|(e, p)| if e >= 0. && p > 0. { Some(e / p) } else { None })
        } else {
            None
        };
        Self {
            percent: number("capacity").map(|v| v.clamp(0., 100.) as u8),
            status,
            hours,
        }
    }
    pub fn label(&self) -> String {
        match (self.percent, self.hours) {
            (Some(p), Some(h)) => format!(
                "Battery {p}% · about {}h {:02}m at current use",
                h.floor() as u32,
                ((h % 1.) * 60.) as u32
            ),
            (Some(p), None) => format!("Battery {p}% · {}", self.status),
            _ => "Battery estimate unavailable".into(),
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn plan_fits_budget_and_uses_remaining_duration() {
        let items = vec![
            Item {
                id: "long".into(),
                kind: "watch".into(),
                seconds: 7200,
                ..Default::default()
            },
            Item {
                id: "book".into(),
                kind: "read".into(),
                ..Default::default()
            },
            Item {
                id: "game".into(),
                kind: "puzzles".into(),
                ..Default::default()
            },
        ];
        let remaining = HashMap::from([("long".into(), 2400)]);
        let plan = lineup(&items, 75, &remaining);
        assert_eq!(plan.iter().map(|s| s.minutes).sum::<u32>(), 75);
        assert_eq!(plan.len(), 3);
        for budget in [0, 3, 5, 20, 40, 60, 180] {
            assert!(
                lineup(&items, budget, &remaining)
                    .iter()
                    .map(|s| s.minutes)
                    .sum::<u32>()
                    <= budget
            );
        }
    }
    #[test]
    fn trip_plans_each_flight_without_repeats() {
        let film = |id: &str, seconds| Item {
            id: id.into(),
            kind: "watch".into(),
            seconds,
            ..Default::default()
        };
        let mut items = vec![film("a", 3000), film("b", 3000), film("c", 1200)];
        items.push(Item {
            id: "chapter".into(),
            kind: "listen".into(),
            seconds: 600,
            ..Default::default()
        });
        let plans = trip(&items, &[60, 60], &HashMap::new());
        let ids: Vec<Vec<&str>> = plans
            .iter()
            .map(|p| p.iter().map(|s| s.item.id.as_str()).collect())
            .collect();
        assert_eq!(ids, vec![vec!["a"], vec!["b"]]);
        assert!(trip(&items, &[], &HashMap::new()).is_empty());
    }
    #[test]
    fn battery_uses_matching_units_and_handles_zero_draw() {
        let root = std::env::temp_dir().join(format!("cabin-battery-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        for (n, v) in [
            ("status", "Discharging"),
            ("capacity", "50"),
            ("energy_now", "30000000"),
            ("power_now", "10000000"),
        ] {
            fs::write(root.join(n), v).unwrap();
        }
        assert_eq!(Battery::from_dir(&root).hours, Some(3.));
        fs::write(root.join("power_now"), "0").unwrap();
        assert_eq!(Battery::from_dir(&root).hours, None);
        fs::remove_dir_all(root).unwrap();
    }
}
