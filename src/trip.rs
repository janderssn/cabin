//! Flight details for the planner, read from a private local file. Nothing
//! here uses the network: the file is written by hand or by a helper outside
//! the app, and boarding passes are files saved next to it.
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Flight {
    pub number: String,
    pub operator: String,
    pub aircraft: String,
    pub from_code: String,
    pub from_city: String,
    pub from_terminal: String,
    pub to_code: String,
    pub to_city: String,
    pub to_terminal: String,
    /// Local time with its UTC offset, e.g. `2026-11-16T14:25:00+00:00`.
    pub departs: String,
    pub arrives: String,
    pub class: String,
    pub seat: String,
}
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Trip {
    pub booking: String,
    pub flights: Vec<Flight>,
}
pub fn dir() -> PathBuf {
    std::env::var_os("CABIN_TRIP")
        .map(PathBuf::from)
        .unwrap_or_else(|| crate::library::home().join(".local/share/cabin/trip"))
}
pub fn load() -> Option<Trip> {
    let text = std::fs::read_to_string(dir().join("trip.json")).ok()?;
    let trip: Trip = serde_json::from_str(&text).ok()?;
    (!trip.flights.is_empty()).then_some(trip)
}
/// A saved boarding pass for a flight: a PDF or picture in the trip folder
/// whose name starts with the flight number, such as `XY101.pdf`.
pub fn boarding_pass(flight: &Flight) -> Option<PathBuf> {
    let wanted = flight.number.replace(' ', "").to_lowercase();
    if wanted.is_empty() {
        return None;
    }
    let mut found: Vec<PathBuf> = std::fs::read_dir(dir())
        .ok()?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            let name = p
                .file_name()
                .map(|n| n.to_string_lossy().replace(' ', "").to_lowercase())
                .unwrap_or_default();
            name.starts_with(&wanted)
                && [".pdf", ".png", ".jpg", ".jpeg"]
                    .iter()
                    .any(|ext| name.ends_with(ext))
        })
        .collect();
    found.sort();
    found.into_iter().next()
}
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}
/// One timestamp split into what the page shows and what sums need.
pub struct Moment {
    /// Seconds since 1970 in UTC.
    pub epoch: i64,
    /// Clock time where the airport is, `14:25`.
    pub clock: String,
    /// `Mon 16 Nov`.
    pub day: String,
}
pub fn moment(text: &str) -> Option<Moment> {
    let number = |range: std::ops::Range<usize>| text.get(range)?.parse::<i64>().ok();
    let (y, mo, d) = (number(0..4)?, number(5..7)?, number(8..10)?);
    let (h, mi) = (number(11..13)?, number(14..16)?);
    let zone = text.get(19..).unwrap_or("Z");
    let offset = if zone.starts_with('+') || zone.starts_with('-') {
        let sign = if zone.starts_with('-') { -1 } else { 1 };
        sign * (zone.get(1..3)?.parse::<i64>().ok()? * 3600
            + zone.get(4..6).and_then(|m| m.parse::<i64>().ok()).unwrap_or(0) * 60)
    } else {
        0
    };
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) || h > 23 || mi > 59 {
        return None;
    }
    let days = days_from_civil(y, mo, d);
    let weekday = ["Thu", "Fri", "Sat", "Sun", "Mon", "Tue", "Wed"][days.rem_euclid(7) as usize];
    let month = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ][(mo - 1) as usize];
    Some(Moment {
        epoch: days * 86400 + h * 3600 + mi * 60 - offset,
        clock: format!("{h:02}:{mi:02}"),
        day: format!("{weekday} {d} {month}"),
    })
}
/// Minutes between two timestamps, never negative.
pub fn minutes_between(from: &str, to: &str) -> Option<u32> {
    let (a, b) = (moment(from)?, moment(to)?);
    Some(((b.epoch - a.epoch).max(0) / 60) as u32)
}
pub fn hours(minutes: u32) -> String {
    format!("{}h {:02}m", minutes / 60, minutes % 60)
}
/// "Departs in 3h 50m", "In the air to Frankfurt", "At Frankfurt · boards in 1h 10m", "Arrived".
pub fn status(trip: &Trip, now: i64) -> String {
    let mut previous_arrival: Option<(i64, &str)> = None;
    for flight in &trip.flights {
        let (Some(dep), Some(arr)) = (moment(&flight.departs), moment(&flight.arrives)) else {
            continue;
        };
        if now < dep.epoch {
            let wait = hours(((dep.epoch - now) / 60) as u32);
            return match previous_arrival {
                Some((_, city)) => format!("At {city} · next flight in {wait}"),
                None => format!("Departs in {wait}"),
            };
        }
        if now < arr.epoch {
            return format!(
                "In the air to {} · {} to go",
                flight.to_city,
                hours(((arr.epoch - now) / 60) as u32)
            );
        }
        previous_arrival = Some((arr.epoch, &flight.to_city));
    }
    "Arrived".into()
}
#[cfg(test)]
mod tests {
    use super::*;
    fn flight(dep: &str, arr: &str, to: &str) -> Flight {
        Flight {
            departs: dep.into(),
            arrives: arr.into(),
            to_city: to.into(),
            ..Default::default()
        }
    }
    #[test]
    fn times_respect_the_airport_offset() {
        let m = moment("2026-11-16T19:45:00+01:00").unwrap();
        assert_eq!((m.clock.as_str(), m.day.as_str()), ("19:45", "Mon 16 Nov"));
        assert_eq!(
            minutes_between("2026-11-16T14:25:00+00:00", "2026-11-16T19:45:00+01:00"),
            Some(260)
        );
        assert!(moment("soon").is_none());
        assert_eq!(moment("1970-01-01T00:00:00Z").unwrap().epoch, 0);
    }
    #[test]
    fn status_follows_the_trip() {
        let trip = Trip {
            booking: String::new(),
            flights: vec![
                flight("2026-11-16T14:25:00+00:00", "2026-11-16T19:45:00+01:00", "Frankfurt"),
                flight("2026-11-16T21:50:00+01:00", "2026-11-16T23:55:00+01:00", "Stockholm"),
            ],
        };
        let start = moment("2026-11-16T14:25:00+00:00").unwrap().epoch;
        assert_eq!(status(&trip, start - 3600), "Departs in 1h 00m");
        assert_eq!(status(&trip, start + 60), "In the air to Frankfurt · 4h 19m to go");
        assert_eq!(status(&trip, start + 270 * 60), "At Frankfurt · next flight in 1h 55m");
        assert_eq!(status(&trip, start + 24 * 3600), "Arrived");
    }
    #[test]
    fn boarding_pass_is_found_by_flight_number() {
        let root = std::env::temp_dir().join(format!("cabin-trip-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("xy101 boarding.PDF"), b"x").unwrap();
        // SAFETY: tests in this module do not read CABIN_TRIP concurrently.
        unsafe { std::env::set_var("CABIN_TRIP", &root) };
        let f = Flight {
            number: "XY101".into(),
            ..Default::default()
        };
        assert!(boarding_pass(&f).is_some());
        let other = Flight {
            number: "XY202".into(),
            ..Default::default()
        };
        assert!(boarding_pass(&other).is_none());
        unsafe { std::env::remove_var("CABIN_TRIP") };
        std::fs::remove_dir_all(root).unwrap();
    }
}
