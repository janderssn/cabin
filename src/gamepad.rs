//! Controller input stays off the render thread and never handles input in a child game.
use gilrs::{Axis, Button, EventType, Gilrs};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Left,
    Right,
    Up,
    Down,
    Accept,
    Back,
    Previous,
    Next,
    Favorite,
    Search,
    Help,
}
pub enum Message {
    Action(Action),
    Connected(Option<String>),
    Error(String),
}
pub struct Input {
    pub events: mpsc::Receiver<Message>,
    enabled: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
}
impl Input {
    pub fn new(ctx: eframe::egui::Context) -> Self {
        let (tx, events) = mpsc::channel();
        let enabled = Arc::new(AtomicBool::new(false));
        let stop = Arc::new(AtomicBool::new(false));
        let (active, ending) = (enabled.clone(), stop.clone());
        thread::spawn(move || {
            let mut pads = match Gilrs::new() {
                Ok(g) => g,
                Err(e) => {
                    let _ = tx.send(Message::Error(e.to_string()));
                    ctx.request_repaint();
                    return;
                }
            };
            let send = |message| {
                let _ = tx.send(message);
                ctx.request_repaint();
            };
            send(Message::Connected(
                pads.gamepads().next().map(|(_, p)| p.name().to_owned()),
            ));
            let mut repeat = Repeat::default();
            let mut was_active = false;
            while !ending.load(Ordering::Relaxed) {
                let focused = active.load(Ordering::Relaxed);
                while let Some(event) = pads.next_event() {
                    match event.event {
                        EventType::Connected | EventType::Disconnected => {
                            send(Message::Connected(
                                pads.gamepads().next().map(|(_, p)| p.name().to_owned()),
                            ));
                            repeat = Repeat::default();
                        }
                        EventType::ButtonPressed(button, _) if focused && was_active => {
                            if let Some(action) = button_action(button) {
                                send(Message::Action(action));
                            }
                        }
                        _ => {}
                    }
                }
                if focused && was_active {
                    let direction = pads.gamepads().find_map(|(_, p)| {
                        if p.is_pressed(Button::DPadLeft) {
                            Some(Action::Left)
                        } else if p.is_pressed(Button::DPadRight) {
                            Some(Action::Right)
                        } else if p.is_pressed(Button::DPadUp) {
                            Some(Action::Up)
                        } else if p.is_pressed(Button::DPadDown) {
                            Some(Action::Down)
                        } else {
                            stick_direction(p.value(Axis::LeftStickX), p.value(Axis::LeftStickY))
                        }
                    });
                    if let Some(action) = repeat.tick(direction, Instant::now()) {
                        send(Message::Action(action));
                    }
                } else {
                    repeat = Repeat::default();
                }
                was_active = focused;
                thread::sleep(Duration::from_millis(16));
            }
        });
        Self {
            events,
            enabled,
            stop,
        }
    }
    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Relaxed);
    }
}
impl Drop for Input {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}
fn button_action(button: Button) -> Option<Action> {
    Some(match button {
        Button::South => Action::Accept,
        Button::East => Action::Back,
        Button::LeftTrigger => Action::Previous,
        Button::RightTrigger => Action::Next,
        Button::West => Action::Favorite,
        Button::North => Action::Search,
        Button::Start => Action::Help,
        _ => return None,
    })
}
fn stick_direction(x: f32, y: f32) -> Option<Action> {
    if x.abs().max(y.abs()) < 0.55 {
        None
    } else if x.abs() > y.abs() {
        Some(if x < 0. { Action::Left } else { Action::Right })
    } else {
        Some(if y > 0. { Action::Up } else { Action::Down })
    }
}
#[derive(Default)]
struct Repeat {
    held: Option<Action>,
    next: Option<Instant>,
}
impl Repeat {
    fn tick(&mut self, direction: Option<Action>, now: Instant) -> Option<Action> {
        if direction != self.held {
            self.held = direction;
            self.next = direction.map(|_| now + Duration::from_millis(350));
            return direction;
        }
        if direction.is_some() && self.next.is_some_and(|next| now >= next) {
            self.next = Some(now + Duration::from_millis(120));
            direction
        } else {
            None
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn controller_mapping_uses_face_positions_and_shoulders() {
        assert_eq!(button_action(Button::South), Some(Action::Accept));
        assert_eq!(button_action(Button::East), Some(Action::Back));
        assert_eq!(button_action(Button::RightTrigger), Some(Action::Next));
        assert_eq!(button_action(Button::DPadLeft), None);
    }
    #[test]
    fn stick_deadzone_and_dominant_axis() {
        assert_eq!(stick_direction(0.3, -0.4), None);
        assert_eq!(stick_direction(0.8, 0.7), Some(Action::Right));
        assert_eq!(stick_direction(0.1, 0.9), Some(Action::Up));
        assert_eq!(stick_direction(0.1, -0.9), Some(Action::Down));
    }
    #[test]
    fn held_navigation_has_delay_repeat_and_release() {
        let mut r = Repeat::default();
        let now = Instant::now();
        assert_eq!(r.tick(Some(Action::Right), now), Some(Action::Right));
        assert_eq!(
            r.tick(Some(Action::Right), now + Duration::from_millis(349)),
            None
        );
        assert_eq!(
            r.tick(Some(Action::Right), now + Duration::from_millis(350)),
            Some(Action::Right)
        );
        assert_eq!(
            r.tick(Some(Action::Right), now + Duration::from_millis(351)),
            None
        );
        assert_eq!(r.tick(None, now + Duration::from_millis(352)), None);
        assert_eq!(
            r.tick(Some(Action::Right), now + Duration::from_millis(353)),
            Some(Action::Right)
        );
    }
}
