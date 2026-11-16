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
