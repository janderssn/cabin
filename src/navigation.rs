#[derive(Clone, Copy)]
pub enum Direction {
    Left,
    Right,
    Up,
    Down,
}
pub fn next_index(
    current: Option<usize>,
    length: usize,
    columns: usize,
    direction: Direction,
) -> Option<usize> {
    if length == 0 {
        return None;
    }
    let Some(i) = current else {
        return Some(0);
    };
    let i = i.min(length - 1);
    let cols = columns.max(1);
    Some(match direction {
        Direction::Left => i.saturating_sub(1),
        Direction::Right => (i + 1).min(length - 1),
        Direction::Up => i.saturating_sub(cols),
        Direction::Down => (i + cols).min(length - 1),
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn grid_navigation_handles_edges_and_short_last_row() {
        assert_eq!(next_index(None, 0, 4, Direction::Down), None);
        assert_eq!(next_index(None, 7, 4, Direction::Down), Some(0));
        assert_eq!(next_index(Some(3), 7, 4, Direction::Down), Some(6));
        assert_eq!(next_index(Some(0), 7, 4, Direction::Left), Some(0));
        assert_eq!(next_index(Some(6), 7, 4, Direction::Up), Some(2));
    }
}

pub fn keyboard_index(current: usize, action: crate::gamepad::Action) -> usize {
    use crate::gamepad::Action::*;
    let rows = [0..10, 10..19, 19..26, 26..30];
    let current = current.min(29);
    let row = rows.iter().position(|r| r.contains(&current)).unwrap();
    let column = current - rows[row].start;
    match action {
        Left => current.saturating_sub(1).max(rows[row].start),
        Right => (current + 1).min(rows[row].end - 1),
        Up | Down => {
            let next = if action == Up {
                row.saturating_sub(1)
            } else {
                (row + 1).min(3)
            };
            (rows[next].start + column).min(rows[next].end - 1)
        }
        _ => current,
    }
}
#[cfg(test)]
mod keyboard_tests {
    use super::*;
    use crate::gamepad::Action::*;
    #[test]
    fn every_search_key_is_reachable_without_leaving_rows() {
        assert_eq!(keyboard_index(9, Down), 18);
        assert_eq!(keyboard_index(18, Down), 25);
        assert_eq!(keyboard_index(25, Down), 29);
        assert_eq!(keyboard_index(26, Left), 26);
        assert_eq!(keyboard_index(29, Up), 22);
        assert_eq!(keyboard_index(0, Up), 0);
    }
}
