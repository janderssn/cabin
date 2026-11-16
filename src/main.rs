mod curation;
mod gamepad;
mod library;
mod navigation;
mod planner;
mod progress;
mod reader;
mod trip;
mod validation;
use eframe::egui::*;
use library::*;
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::PathBuf,
    process::{Child, Stdio},
    sync::mpsc,
    time::{Duration, Instant},
};
use winit::platform::wayland::EventLoopBuilderExtWayland;
use winit::platform::x11::EventLoopBuilderExtX11;
const BG: Color32 = Color32::from_rgb(15, 19, 25);
const PANEL: Color32 = Color32::from_rgb(21, 27, 35);
const CARD: Color32 = Color32::from_rgb(28, 35, 45);
const TEXT: Color32 = Color32::from_rgb(237, 239, 242);
const MUTED: Color32 = Color32::from_rgb(149, 163, 181);
const BORDER: Color32 = Color32::from_rgb(43, 53, 66);
fn accent() -> Color32 {
    let p = home().join(".local/state/omarchy/current/theme/colors.toml");
    if let Ok(s) = fs::read_to_string(p) {
        for l in s.lines() {
            if l.trim().starts_with("accent") {
                if let Some(i) = l.find('#') {
                    if l.len() >= i + 7 {
                        if let Ok(v) = u32::from_str_radix(&l[i + 1..i + 7], 16) {
                            return Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8);
                        }
                    }
                }
            }
        }
    }
    Color32::from_rgb(235, 190, 112)
}
// Omarchy suppresses application maximize/fullscreen requests. Address only our
// own mapped window through the compositor; never change global window rules.
fn desktop_fullscreen(action: &str) -> bool {
    let clients = std::process::Command::new("hyprctl")
        .args(["clients", "-j"])
        .output()
        .ok()
        .and_then(|o| serde_json::from_slice::<serde_json::Value>(&o.stdout).ok());
    let address = clients
        .as_ref()
        .and_then(|v| v.as_array())
        .and_then(|a| {
            a.iter()
                .find(|c| c["pid"].as_u64() == Some(std::process::id() as u64))
        })
        .and_then(|c| c["address"].as_str());
    if let Some(address) = address {
        let dispatch = format!(
            "hl.dsp.window.fullscreen({{mode=\"fullscreen\",action=\"{action}\",window=\"address:{address}\"}})"
        );
        return std::process::Command::new("hyprctl")
            .args(["dispatch", &dispatch])
            .output()
            .is_ok_and(|o| o.status.success());
    }
    false
}
struct Cabin {
    gamepad: gamepad::Input,
    controller: Option<String>,
    keyboard_key: usize,
    controller_scroll: f32,
    items: Vec<Item>,
    progress: progress::Progress,
    history: History,
    section: String,
    query: String,
    detail: Option<String>,
    textures: HashMap<String, TextureHandle>,
    pending: HashSet<String>,
    image_jobs: mpsc::Sender<String>,
    image_results: mpsc::Receiver<(String, Option<ColorImage>)>,
    nav_cards: Vec<String>,
    nav_columns: usize,
    focused_card: Option<String>,
    focus_ring: bool,
    scroll_focus: bool,
    battery: planner::Battery,
    gallery_index: usize,
    accent: Color32,
    notice: Option<(String, bool, Instant)>,
    children: Vec<(String, Child)>,
    help: bool,
    audio_scan: Option<Child>,
    touch_keyboard: bool,
    show_unverified: bool,
    travel_enabled: bool,
    fullscreen_requested: bool,
    started: Instant,
    capture_requested: bool,
    focus_search: bool,
    screenshot: Option<PathBuf>,
    /// Scripted moves for automated screenshots (`--moves down,down,up`).
    scripted_moves: Vec<navigation::Direction>,
    scripted_done: usize,
    /// Which flight's line-up holds the focus in the planner.
    focused_leg: usize,
    /// The magazine open in the built-in reader, with its page pictures.
    reader: Option<reader::Reader>,
    /// Item to open as soon as the window is up (`--read ITEM_ID`).
    open_on_start: Option<String>,
    ctx: Context,
    reader_pages: HashMap<u32, TextureHandle>,
    frames: u32,
    theme_check: Instant,
}
impl Cabin {
    fn new(
        cc: &eframe::CreationContext<'_>,
        section: String,
        detail: Option<String>,
        screenshot: Option<PathBuf>,
        scripted_moves: Vec<navigation::Direction>,
        open_on_start: Option<String>,
    ) -> Self {
        let mut visuals = Visuals::dark();
        visuals.panel_fill = BG;
        visuals.window_fill = PANEL;
        visuals.override_text_color = Some(TEXT);
        visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, BORDER);
        visuals.widgets.inactive.bg_fill = CARD;
        visuals.widgets.hovered.bg_fill = Color32::from_rgb(46, 56, 70);
        visuals.widgets.active.bg_fill = Color32::from_rgb(61, 71, 87);
        visuals.selection.bg_fill = accent().gamma_multiply(0.3);
        cc.egui_ctx.set_visuals(visuals);
        // XWayland reports Xft DPI separately from the compositor's physical scale.
        // Match the actual output scale to avoid oversized controls on this desktop.
        if !std::env::args().any(|a| a == "--wayland") {
            cc.egui_ctx.set_zoom_factor(0.75);
        }
        cc.egui_ctx.style_mut(|s| {
            s.spacing.item_spacing = vec2(12., 12.);
            s.spacing.button_padding = vec2(20., 14.);
            s.spacing.interact_size = vec2(52., 52.);
            s.animation_time = 0.12;
            s.text_styles
                .insert(TextStyle::Body, FontId::proportional(20.));
            s.text_styles
                .insert(TextStyle::Button, FontId::proportional(20.));
        });
        let (items, notice) = match load() {
            Ok(i) => (i, None),
            Err(e) => (vec![], Some((e, true, Instant::now()))),
        };
        let (image_jobs, jobs) = mpsc::channel::<String>();
        let (results, image_results) = mpsc::channel();
        let repaint = cc.egui_ctx.clone();
        std::thread::spawn(move || {
            while let Ok(path) = jobs.recv() {
                let decoded = image::open(&path).ok().map(|img| {
                    let img = img.thumbnail(960, 720).to_rgba8();
                    ColorImage::from_rgba_unmultiplied(
                        [img.width() as usize, img.height() as usize],
                        img.as_raw(),
                    )
                });
                if results.send((path, decoded)).is_err() {
                    break;
                }
                repaint.request_repaint();
            }
        });
        Self {
            gamepad: gamepad::Input::new(cc.egui_ctx.clone()),
            controller: None,
            keyboard_key: 0,
            controller_scroll: 0.,
            pending: HashSet::new(),
            image_jobs,
            image_results,
            progress: progress::Progress::new(&items),
            items,
            history: load_history(),
            section,
            query: String::new(),
            detail,
            textures: HashMap::new(),
            nav_cards: Vec::new(),
            nav_columns: 1,
            focused_card: None,
            focus_ring: false,
            scroll_focus: false,
            battery: planner::Battery::read(),
            gallery_index: 0,
            accent: accent(),
            notice,
            children: vec![],
            help: false,
            audio_scan: None,
            show_unverified: false,
            travel_enabled: curation::travel_enabled(),
            touch_keyboard: std::env::args().any(|a| a == "--keyboard"),
            fullscreen_requested: std::env::args().any(|a| a == "--windowed"),
            started: Instant::now(),
            capture_requested: false,
            focus_search: false,
            screenshot,
            scripted_moves,
            scripted_done: 0,
            focused_leg: 0,
            reader: None,
            open_on_start,
            ctx: cc.egui_ctx.clone(),
            reader_pages: HashMap::new(),
            frames: 0,
            theme_check: Instant::now(),
        }
    }
    fn texture(&mut self, ctx: &Context, path: &str) -> Option<TextureHandle> {
        if path.is_empty() {
            return None;
        }
        if let Some(t) = self.textures.get(path) {
            return Some(t.clone());
        }
        if self.pending.insert(path.into()) {
            let _ = self.image_jobs.send(path.into());
        }
        let _ = ctx;
        None
    }

    /// `contain` shows the whole picture; otherwise it fills the frame and the
    /// overflow is cropped. `keep_top` crops from the bottom only, so a
    /// magazine cover keeps its masthead.
    fn picture(&mut self, ui: &Ui, rect: Rect, path: &str, contain: bool) {
        self.picture_fit(ui, rect, path, contain, false);
    }
    fn picture_fit(&mut self, ui: &Ui, rect: Rect, path: &str, contain: bool, keep_top: bool) {
        ui.painter()
            .rect_filled(rect, 10, Color32::from_rgb(9, 13, 18));
        if let Some(t) = self.texture(ui.ctx(), path) {
            let aspect = t.size_vec2().x / t.size_vec2().y;
            let target = rect.width() / rect.height();
            let (dest, uv) = if contain {
                let size = if aspect > target {
                    vec2(rect.width(), rect.width() / aspect)
                } else {
                    vec2(rect.height() * aspect, rect.height())
                };
                (
                    Rect::from_center_size(rect.center(), size),
                    Rect::from_min_max(pos2(0., 0.), pos2(1., 1.)),
                )
            } else {
                let uv = if aspect > target {
                    let f = target / aspect;
                    Rect::from_min_max(pos2((1. - f) / 2., 0.), pos2((1. + f) / 2., 1.))
                } else {
                    let f = aspect / target;
                    let top = if keep_top { 0. } else { (1. - f) / 2. };
                    Rect::from_min_max(pos2(0., top), pos2(1., top + f))
                };
                (rect, uv)
            };
            ui.painter().image(t.id(), dest, uv, Color32::WHITE);
        } else {
            ui.painter()
                .circle_stroke(rect.center(), 26., Stroke::new(2.0_f32, BORDER));
            ui.painter().text(
                rect.center(),
                Align2::CENTER_CENTER,
                "C",
                FontId::proportional(30.),
                self.accent,
            );
        }
    }
    fn save(&mut self) {
        if let Err(e) = save_history(&self.history) {
            self.notice = Some((e, true, Instant::now()));
        }
    }
    fn favorite(&mut self, id: &str) {
        if !self.history.favorites.remove(id) {
            self.history.favorites.insert(id.into());
        }
        self.save();
    }
    fn close_reader(&mut self) {
        if let Some(r) = self.reader.take() {
            self.history.pages.insert(r.id, r.page);
            self.save();
        }
        self.reader_pages.clear();
    }
    /// Keys and buttons for the reader. Returns true while a magazine is open.
    fn reader_input(&mut self, ctx: &Context) -> bool {
        let Some(r) = self.reader.as_mut() else {
            return false;
        };
        let pressed = |key: Key| ctx.input_mut(|i| i.consume_key(Modifiers::NONE, key));
        if pressed(Key::ArrowRight) || pressed(Key::PageDown) || pressed(Key::Space) {
            r.go(1);
        }
        if pressed(Key::ArrowLeft) || pressed(Key::PageUp) {
            r.go(-1);
        }
        if pressed(Key::Home) {
            r.go(-100000);
        }
        if pressed(Key::End) {
            r.go(100000);
        }
        if pressed(Key::ArrowDown) {
            self.controller_scroll = -260.;
        }
        if pressed(Key::ArrowUp) {
            self.controller_scroll = 260.;
        }
        if pressed(Key::Plus) || pressed(Key::Equals) {
            r.zoom = (r.zoom + 1).min(3);
        }
        if pressed(Key::Minus) {
            r.zoom = r.zoom.saturating_sub(1);
        }
        let close = pressed(Key::Escape) || pressed(Key::Backspace) || pressed(Key::Q);
        pressed(Key::Enter);
        if close {
            self.close_reader();
        }
        true
    }
    fn reader_view(&mut self, ui: &mut Ui) {
        let Some(r) = self.reader.as_mut() else {
            return;
        };
        while let Ok((page, image)) = r.results.try_recv() {
            if let Some(image) = image {
                let name = format!("page-{}-{page}", r.id);
                self.reader_pages
                    .insert(page, ui.ctx().load_texture(name, image, TextureOptions::LINEAR));
            }
        }
        for far in r.forget_far() {
            self.reader_pages.remove(&far);
        }
        r.request(r.page);
        r.request(r.page + 1);
        let (mut turn, mut zoom, mut close) = (0_i64, None, false);
        // One bar: back on the left, title and page in the middle, size and page turns on the right.
        let (bar, _) = ui.allocate_exact_size(vec2(ui.available_width(), 54.), Sense::hover());
        let y = bar.center().y;
        let at = |x: f32, w: f32| Rect::from_center_size(pos2(x, y), vec2(w, 50.));
        if ui.put(at(bar.left() + 60., 120.), Button::new("Back")).clicked() {
            close = true;
        }
        let right = bar.right();
        let title = ui
            .painter()
            .layout_no_wrap(r.title.clone(), FontId::proportional(22.), TEXT);
        ui.painter()
            .with_clip_rect(Rect::from_min_max(
                pos2(bar.left() + 144., bar.top()),
                pos2(bar.center().x - 300., bar.bottom()),
            ))
            .galley(pos2(bar.left() + 144., y - title.size().y / 2.), title, TEXT);
        if ui.put(at(right - 60., 120.), Button::new("Next")).clicked() {
            turn = 1;
        }
        if ui.put(at(right - 190., 120.), Button::new("Previous")).clicked() {
            turn = -1;
        }
        ui.painter().text(
            pos2(right - 270., y),
            Align2::RIGHT_CENTER,
            format!("Page {} of {}", r.page + 1, r.pages),
            FontId::proportional(18.),
            MUTED,
        );
        for (n, label) in ["Whole page", "Read", "Large", "Full width"].into_iter().enumerate() {
            // The four sizes sit together in the middle of the bar.
            let x = bar.center().x - 213. + n as f32 * 142.;
            let chosen = r.zoom == n;
            let button = Button::new(RichText::new(label).size(16.).color(if chosen { BG } else { TEXT }))
                .fill(if chosen { self.accent } else { CARD });
            if ui.put(at(x, 134.), button).clicked() {
                zoom = Some(n);
            }
        }
        ui.add_space(12.);
        let (page, zoom_now, top) = (r.page, r.zoom, r.scroll_top);
        r.scroll_top = false;
        let texture = self.reader_pages.get(&page).cloned();
        let scroll = std::mem::take(&mut self.controller_scroll);
        let mut area = ScrollArea::vertical()
            .id_salt(("reader", page))
            .auto_shrink([false, false]);
        if top {
            area = area.vertical_scroll_offset(0.);
        }
        area.show(ui, |ui| {
            if scroll != 0. {
                ui.scroll_with_delta(vec2(0., scroll));
            }
            let (wide, tall) = (ui.available_width(), ui.available_height());
            let Some(t) = texture else {
                ui.add_space(tall * 0.4);
                ui.vertical_centered(|ui| ui.label(RichText::new("Preparing page…").color(MUTED)));
                return;
            };
            let aspect = t.size_vec2().x / t.size_vec2().y;
            let width = match zoom_now {
                0 => (tall * aspect).min(wide),
                1 => wide.min(900.),
                2 => wide.min(1150.),
                _ => wide,
            };
            let size = vec2(width, width / aspect);
            // Centred, so every page sits in the same place whatever its size.
            ui.horizontal(|ui| {
                ui.add_space(((wide - width) / 2.).max(0.));
                let (rect, response) = ui.allocate_exact_size(size, Sense::click());
                ui.painter().image(
                    t.id(),
                    rect,
                    Rect::from_min_max(pos2(0., 0.), pos2(1., 1.)),
                    Color32::WHITE,
                );
                // Tap the right half for the next page, the left half to go back.
                if let Some(pos) = response.interact_pointer_pos().filter(|_| response.clicked()) {
                    turn = if pos.x > rect.center().x { 1 } else { -1 };
                }
            });
        });
        if let Some(r) = self.reader.as_mut() {
            if turn != 0 {
                r.go(turn);
            }
            if let Some(z) = zoom {
                r.zoom = z;
            }
        }
        if close {
            self.close_reader();
        }
    }
    fn launch(&mut self, item: &Item) {
        // Magazines open in the built-in reader; other viewers show these scans far too small.
        if item.kind == "read" && item.path.to_lowercase().ends_with(".pdf") && std::path::Path::new(&item.path).is_file()
        {
            let page = self.history.pages.get(&item.id).copied().unwrap_or(0);
            let ctx = self.ctx.clone();
            self.reader_pages.clear();
            self.reader = Some(reader::Reader::open(&ctx, &item.id, &item.title, &item.path, item.pages as u32, page));
            self.history.recent.retain(|x| x != &item.id);
            self.history.recent.insert(0, item.id.clone());
            self.history.recent.truncate(24);
            self.save();
            return;
        }
        let result = (|| -> Result<Child, String> {
            let mut c = build_command(item)?;
            fs::create_dir_all(state_dir()).map_err(|e| e.to_string())?;
            let f = fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(state_dir().join("launch.log"))
                .map_err(|e| e.to_string())?;
            c.stdout(Stdio::from(f.try_clone().map_err(|e| e.to_string())?))
                .stderr(Stdio::from(f));
            c.spawn().map_err(|e| e.to_string())
        })();
        match result {
            Ok(child) => {
                self.children.push((item.title.clone(), child));
                self.history.recent.retain(|x| x != &item.id);
                self.history.recent.insert(0, item.id.clone());
                self.history.recent.truncate(24);
                self.save();
                self.notice = Some((format!("Opened {}", item.title), false, Instant::now()));
            }
            Err(e) => self.notice = Some((e, true, Instant::now())),
        }
    }
    fn move_focus(&mut self, direction: navigation::Direction) {
        let current = self
            .focused_card
            .as_ref()
            .and_then(|id| self.nav_cards.iter().position(|x| x == id));
        // Up from the first row has no card to go to: show the top of the page
        // (the heading and featured item) instead of staying centred on the card.
        if matches!(direction, navigation::Direction::Up)
            && current.is_some_and(|i| i < self.nav_columns.max(1))
        {
            self.focus_ring = true;
            self.scroll_focus = false;
            self.controller_scroll = 1.0e6;
            return;
        }
        if let Some(next) =
            navigation::next_index(current, self.nav_cards.len(), self.nav_columns, direction)
        {
            self.focused_card = Some(self.nav_cards[next].clone());
            self.focus_ring = true;
            self.scroll_focus = true;
        }
    }
    fn keyboard_press(&mut self, index: usize) {
        if let Some(key) = "qwertyuiopasdfghjklzxcvbnm".chars().nth(index) {
            self.query.push(key);
            self.detail = None;
        } else {
            match index {
                26 => self.query.clear(),
                27 => self.query.push(' '),
                28 => {
                    self.query.pop();
                }
                29 => self.touch_keyboard = false,
                _ => {}
            }
        }
    }
    fn controller_action(&mut self, ctx: &Context, action: gamepad::Action) {
        use gamepad::Action::*;
        if let Some(r) = self.reader.as_mut() {
            match action {
                Right | Next | Accept => r.go(1),
                Left | Previous => r.go(-1),
                Down => self.controller_scroll = -260.,
                Up => self.controller_scroll = 260.,
                Back => self.close_reader(),
                _ => {}
            }
            return;
        }
        self.focus_ring = true;
        ctx.memory_mut(|m| m.surrender_focus(Id::new("search")));
        if action == Back {
            if self.help {
                self.help = false;
            } else if self.touch_keyboard {
                self.touch_keyboard = false;
            } else if self.detail.is_some() {
                self.detail = None;
                self.scroll_focus = true;
            } else if !self.query.is_empty() {
                self.query.clear();
            } else {
                self.section = "home".into();
            }
            return;
        }
        if action == Help {
            self.help = !self.help;
            return;
        }
        if self.help {
            return;
        }
        if action == Search {
            self.touch_keyboard = !self.touch_keyboard;
            self.detail = None;
            return;
        }
        if self.touch_keyboard {
            if action == Accept {
                self.keyboard_press(self.keyboard_key);
            } else {
                self.keyboard_key = navigation::keyboard_index(self.keyboard_key, action);
            }
            return;
        }
        if matches!(action, Previous | Next) {
            let sections = [
                "home",
                "planner",
                "travel",
                "watch",
                "listen",
                "puzzles",
                "atari",
                "read",
                "favorites",
                "recent",
            ];
            let i = sections
                .iter()
                .position(|s| *s == self.section)
                .unwrap_or(0);
            self.section = sections[(i + if action == Next {
                1
            } else {
                sections.len() - 1
            }) % sections.len()]
            .into();
            self.detail = None;
            self.query.clear();
            self.focused_card = None;
            return;
        }
        if self.section == "planner" && self.detail.is_none() && matches!(action, Left | Right) {
            // Adjusts the flight whose line-up holds the focus (the first by default).
            let leg = self.focused_leg.min(self.history.plan.len().saturating_sub(1));
            if let Some(minutes) = self.history.plan.get_mut(leg) {
                *minutes = if action == Left {
                    minutes.saturating_sub(15).max(15)
                } else {
                    (*minutes + 15).min(1440)
                };
                self.save();
            }
            return;
        }
        if self.detail.is_some() {
            match action {
                Up => {
                    self.controller_scroll = 240.;
                    return;
                }
                Down => {
                    self.controller_scroll = -240.;
                    return;
                }
                Left => {
                    self.gallery_index = self.gallery_index.saturating_sub(1);
                    return;
                }
                Right => {
                    if let Some(item) = self
                        .items
                        .iter()
                        .find(|i| Some(&i.id) == self.detail.as_ref())
                    {
                        self.gallery_index =
                            (self.gallery_index + 1).min(item.gallery.len().saturating_sub(1));
                    }
                    return;
                }
                _ => {}
            }
        }
        match action {
            Left | Right | Up | Down if self.detail.is_none() => self.move_focus(match action {
                Left => navigation::Direction::Left,
                Right => navigation::Direction::Right,
                Up => navigation::Direction::Up,
                _ => navigation::Direction::Down,
            }),
            Accept => {
                if let Some(id) = self.detail.clone() {
                    if let Some(item) = self.items.iter().find(|i| i.id == id).cloned() {
                        self.launch(&item);
                        self.gamepad.set_enabled(false);
                    }
                } else if let Some(id) = self
                    .focused_card
                    .clone()
                    .filter(|id| self.nav_cards.contains(id))
                    .or_else(|| self.nav_cards.first().cloned())
                {
                    self.focused_card = Some(id.clone());
                    self.detail = Some(id);
                    self.gallery_index = 0;
                }
            }
            Favorite => {
                if let Some(id) = self.detail.clone().or_else(|| self.focused_card.clone()) {
                    if self.history.favorites.contains(&id) {
                        self.history.favorites.retain(|i| i != &id);
                    } else {
                        self.history.favorites.insert(id);
                    }
                    self.save();
                }
            }
            _ => {}
        }
    }
    fn search_toolbar(&mut self, ui: &mut Ui) {
        let wide = ui.available_width() >= 900.;
        let compact = ui.available_width() < 560.;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 12.;
            if wide {
                ui.allocate_ui_with_layout(vec2(220., 56.), Layout::top_down(Align::Min), |ui| {
                    ui.label(RichText::new("CABIN").size(22.).strong());
                    ui.label(
                        RichText::new("Your offline collection")
                            .size(14.)
                            .color(MUTED),
                    );
                });
                ui.add_space((ui.available_width() - 730.).max(16.));
            }
            let search_width = (ui.available_width() - if compact { 0. } else { 204. }).max(100.);
            let (rect, field) = ui.allocate_exact_size(vec2(search_width, 56.), Sense::click());
            let id = Id::new("search");
            let focused = ui.memory(|m| m.has_focus(id));
            ui.painter().rect(
                rect,
                16,
                PANEL,
                Stroke::new(
                    if focused { 2.0_f32 } else { 1.0_f32 },
                    if focused { self.accent } else { BORDER },
                ),
                StrokeKind::Inside,
            );
            let center = pos2(rect.left() + 24., rect.center().y - 2.);
            ui.painter().circle_stroke(
                center,
                7.,
                Stroke::new(2.0_f32, if focused { self.accent } else { MUTED }),
            );
            ui.painter().line_segment(
                [center + vec2(5., 5.), center + vec2(11., 11.)],
                Stroke::new(2.0_f32, MUTED),
            );
            let trailing = if self.query.is_empty() {
                if search_width > 320. { 72. } else { 12. }
            } else {
                52.
            };
            let input_rect = Rect::from_min_max(
                pos2(rect.left() + 48., rect.top()),
                pos2(rect.right() - trailing, rect.bottom()),
            );
            let mut input_ui = ui.new_child(
                UiBuilder::new()
                    .max_rect(input_rect)
                    .layout(Layout::left_to_right(Align::Center)),
            );
            let response = input_ui.add(
                TextEdit::singleline(&mut self.query)
                    .id(id)
                    .frame(false)
                    .desired_width(input_rect.width())
                    .font(FontId::proportional(20.))
                    .text_color(TEXT)
                    .hint_text(RichText::new("Search collection").color(MUTED)),
            );
            if self.focus_search || field.clicked() {
                response.request_focus();
                self.focus_search = false;
            }
            if response.changed() {
                self.detail = None;
            }
            if self.query.is_empty() {
                if search_width > 320. {
                    ui.painter().text(
                        pos2(rect.right() - 16., rect.center().y),
                        Align2::RIGHT_CENTER,
                        "Ctrl F",
                        FontId::proportional(14.),
                        MUTED,
                    );
                }
            } else {
                let clear = Rect::from_center_size(
                    pos2(rect.right() - 27., rect.center().y),
                    vec2(44., 44.),
                );
                let r = ui
                    .interact(clear, Id::new("clear_search"), Sense::click())
                    .on_hover_text("Clear search");
                if r.hovered() {
                    ui.painter().rect_filled(clear.shrink(3.), 12, CARD);
                }
                let c = clear.center();
                for sign in [-1., 1.] {
                    ui.painter().line_segment(
                        [c + vec2(-5., sign * -5.), c + vec2(5., sign * 5.)],
                        Stroke::new(2.0_f32, MUTED),
                    );
                }
                if r.clicked() {
                    self.query.clear();
                    response.request_focus();
                }
            }
            if !compact {
                self.toolbar_actions(ui);
            }
        });
        if compact {
            ui.horizontal(|ui| self.toolbar_actions(ui));
        }
    }
    fn toolbar_actions(&mut self, ui: &mut Ui) {
        for (label, active) in [("Keyboard", self.touch_keyboard), ("Reload", false)] {
            let r = ui.add_sized(
                [if label == "Keyboard" { 94. } else { 66. }, 56.],
                Button::new(RichText::new(label).size(16.).color(if active {
                    self.accent
                } else {
                    MUTED
                }))
                .fill(if active { CARD } else { BG })
                .corner_radius(14.),
            );
            if r.clicked() {
                if label == "Keyboard" {
                    self.touch_keyboard = !self.touch_keyboard;
                } else {
                    match load() {
                        Ok(i) => {
                            self.progress = progress::Progress::new(&i);
                            self.travel_enabled = curation::travel_enabled();
                            self.items = i;
                        }
                        Err(e) => self.notice = Some((e, true, Instant::now())),
                    }
                }
            }
        }
    }
    fn sidebar(&mut self, ctx: &Context) {
        TopBottomPanel::bottom("touch_navigation")
            .frame(Frame::new().fill(PANEL).inner_margin(12))
            .show(ctx, |ui| {
                if self.controller.is_some() {
                    ui.label(RichText::new(if self.touch_keyboard {"D-pad  Choose key     A / ×  Type     B / ○  Done"} else if self.section=="planner" && self.detail.is_none() {"← / →  Flight length     ↑ / ↓  Line-up     A / ×  Open     LB / RB  Sections"} else {"D-pad / stick  Browse     A / ×  Open / play     B / ○  Back     LB / RB  Sections     X / □  Favorite     Y / △  Search"}).size(16.).color(MUTED));
                }
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = vec2(8., 8.);
                    for (id, label) in [
                        ("home", "Discover"),
                        ("planner", "Flight"),
                        ("travel", "Travel"),
                        ("watch", "Watch"),
                        ("listen", "Listen"),
                        ("puzzles", "Puzzles"),
                        ("atari", "Atari"),
                        ("read", "Read"),
                        ("favorites", "Favorites"),
                        ("recent", "Recent"),
                    ] {
                        let selected = self.section == id;
                        if ui
                            .add_sized(
                                [112., 54.],
                                Button::new(RichText::new(label).size(20.).color(if selected {
                                    BG
                                } else {
                                    TEXT
                                }))
                                .fill(if selected {
                                    self.accent
                                } else {
                                    CARD
                                }),
                            )
                            .clicked()
                        {
                            // Leaving for another section also leaves the magazine.
                            self.close_reader();
                            self.section = id.into();
                            self.detail = None;
                            self.query.clear();
                        }
                    }
                    if ui.add_sized([80., 54.], Button::new("Help")).clicked() {
                        self.help = true;
                    }
                    if ui.add_sized([80., 54.], Button::new("Quit")).clicked() {
                        ctx.send_viewport_cmd(ViewportCommand::Close);
                    }
                });
            });
    }
    fn card(&mut self, ui: &mut Ui, item: &Item, width: f32) {
        let (rect, response) = ui.allocate_exact_size(vec2(width, 320.), Sense::click());
        self.nav_cards.push(item.id.clone());
        let focused = self.focus_ring && self.focused_card.as_ref() == Some(&item.id);
        if focused && self.scroll_focus {
            ui.scroll_to_rect(rect, Some(Align::Center));
            self.scroll_focus = false;
        }
        if !ui.is_rect_visible(rect) {
            return;
        }
        let fill = if response.hovered() {
            Color32::from_rgb(35, 44, 56)
        } else {
            CARD
        };
        ui.painter().rect_filled(rect, 14, fill);
        if response.hovered() || focused {
            ui.painter().rect_stroke(
                rect,
                14,
                Stroke::new(if focused { 3_f32 } else { 1_f32 }, self.accent),
                StrokeKind::Inside,
            );
        }
        let image_rect = Rect::from_min_size(rect.min + vec2(8., 8.), vec2(width - 16., 170.));
        // Album and podcast art is shown whole; everything else fills the frame.
        self.picture_fit(ui, image_rect, &item.image, item.kind == "listen", item.kind == "read");
        let badge = Rect::from_min_size(
            image_rect.left_bottom() + vec2(8., -34.),
            vec2((item.meta.len() as f32 * 9. + 20.).min(width - 40.), 28.),
        );
        ui.painter()
            .rect_filled(badge, 5, Color32::from_black_alpha(205));
        ui.painter().text(
            badge.left_center() + vec2(8., 0.),
            Align2::LEFT_CENTER,
            &item.meta,
            FontId::proportional(16.),
            TEXT,
        );
        let p = rect.min + vec2(15., 192.);
        ui.painter().text(
            p,
            Align2::LEFT_TOP,
            item.tag.to_uppercase(),
            FontId::proportional(14.),
            self.accent,
        );
        let title = ui.painter().layout(
            item.title.clone(),
            FontId::proportional(23.),
            TEXT,
            width - 30.,
        );
        ui.painter()
            .with_clip_rect(Rect::from_min_size(
                p + vec2(0., 25.),
                vec2(width - 30., 58.),
            ))
            .galley(p + vec2(0., 25.), title, TEXT);
        let sub =
            ui.painter()
                .layout_no_wrap(item.subtitle.clone(), FontId::proportional(16.), MUTED);
        ui.painter()
            .with_clip_rect(Rect::from_min_size(
                rect.min + vec2(15., 282.),
                vec2(width - 30., 26.),
            ))
            .galley(rect.min + vec2(15., 282.), sub, MUTED);
        if self.history.favorites.contains(&item.id) {
            ui.painter().text(
                image_rect.right_top() + vec2(-12., 10.),
                Align2::RIGHT_TOP,
                "★",
                FontId::proportional(19.),
                self.accent,
            );
        }
        if item.seconds > 0 && self.progress.seconds(item) > 0. {
            let bar = Rect::from_min_size(
                pos2(rect.min.x + 12., rect.max.y - 7.),
                vec2(rect.width() - 24., 4.),
            );
            ui.painter().rect_filled(bar, 2, BORDER);
            let fill = Rect::from_min_size(
                bar.min,
                vec2(
                    bar.width() * (self.progress.seconds(item) / item.seconds as f64) as f32,
                    bar.height(),
                ),
            );
            ui.painter().rect_filled(fill, 2, self.accent);
        }
        if response.clicked() {
            self.detail = Some(item.id.clone());
            self.gallery_index = 0;
        }
        if self.screenshot.is_none() {
            response.on_hover_text(format!("{} · {}", item.title, action_label(item)));
        }
    }
    fn grid(&mut self, ui: &mut Ui, items: Vec<Item>) {
        let items: Vec<_> = items
            .into_iter()
            .filter(|i| visible_game(i, self.show_unverified))
            .collect();
        if items.is_empty() {
            ui.add_space(70.);
            ui.vertical_centered(|ui| {
                ui.heading(if self.section == "travel" {
                    "Let’s find your kind of escape"
                } else {
                    "Nothing here just yet"
                });
                ui.label(
                    RichText::new(if self.section == "favorites" {
                        "Open an item and save it to your favorites."
                    } else if self.section == "recent" {
                        "The things you open will appear here."
                    } else {
                        if self.section == "travel" {
                            "The previous picks have been set aside."
                        } else {
                            "Try another search or category."
                        }
                    })
                    .color(MUTED),
                );
            });
            return;
        }
        let n = ((ui.available_width() + 16.) / 330.).floor().max(1.) as usize;
        self.nav_columns = n;
        let width = (ui.available_width() - 16. * (n - 1) as f32) / n as f32;
        for chunk in items.chunks(n) {
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 16.;
                for item in chunk {
                    self.card(ui, item, width);
                }
            });
            ui.add_space(4.);
        }
    }
    fn hero(&mut self, ui: &mut Ui, item: Item) {
        let width = ui.available_width();
        let (rect, _) = ui.allocate_exact_size(vec2(width, 262.), Sense::hover());
        ui.painter().rect_filled(rect, 16, CARD);
        let pic = Rect::from_min_max(
            pos2(rect.min.x + width * 0.49, rect.min.y + 8.),
            rect.max - vec2(8., 8.),
        );
        self.picture_fit(ui, pic, &item.image, false, item.kind == "read");
        let left = Rect::from_min_max(
            rect.min + vec2(26., 25.),
            pos2(pic.min.x - 20., rect.max.y - 16.),
        );
        ui.scope_builder(UiBuilder::new().max_rect(left), |ui| {
            ui.label(
                RichText::new(format!("FEATURED / {}", item.tag.to_uppercase()))
                    .size(16.)
                    .color(self.accent)
                    .strong(),
            );
            ui.add_space(7.);
            ui.label(RichText::new(&item.title).size(29.).strong());
            ui.add_space(6.);
            ui.label(RichText::new(&item.subtitle).size(16.).color(MUTED));
            ui.add_space(10.);
            ui.horizontal(|ui| {
                if ui
                    .add(
                        Button::new(RichText::new(action_label(&item)).color(BG).strong())
                            .fill(self.accent),
                    )
                    .clicked()
                {
                    self.launch(&item);
                }
                if ui
                    .add(
                        Button::new("Details")
                            .fill(Color32::TRANSPARENT)
                            .stroke(Stroke::new(1.0_f32, BORDER)),
                    )
                    .clicked()
                {
                    self.detail = Some(item.id.clone());
                    self.gallery_index = 0;
                }
            });
        });
        ui.advance_cursor_after_rect(rect);
    }
    /// One flight drawn as a ticket: codes, times, terminals and the boarding pass.
    fn flight_card(&mut self, ui: &mut Ui, rect: Rect, flight: &trip::Flight) {
        let p = ui.painter().clone();
        p.rect_filled(rect, 14, CARD);
        p.rect_stroke(rect, 14, Stroke::new(1.0_f32, BORDER), StrokeKind::Inside);
        let (l, r, t) = (rect.left() + 22., rect.right() - 22., rect.top());
        let text = |pos: Pos2, align: Align2, s: &str, size: f32, color: Color32| {
            p.text(pos, align, s, FontId::proportional(size), color);
        };
        text(pos2(l, t + 26.), Align2::LEFT_CENTER, &flight.number, 18., self.accent);
        let carrier = [flight.operator.as_str(), flight.aircraft.as_str()]
            .into_iter()
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("  ·  ");
        text(pos2(r, t + 26.), Align2::RIGHT_CENTER, &carrier, 15., MUTED);
        // Codes with the time in the air on the line between them.
        let y = t + 74.;
        text(pos2(l, y), Align2::LEFT_CENTER, &flight.from_code, 40., TEXT);
        text(pos2(r, y), Align2::RIGHT_CENTER, &flight.to_code, 40., TEXT);
        if rect.width() > 380. {
            p.line_segment(
                [pos2(l + 104., y), pos2(r - 104., y)],
                Stroke::new(1.5_f32, BORDER),
            );
            if let Some(m) = trip::minutes_between(&flight.departs, &flight.arrives) {
                let label = trip::hours(m);
                let half = label.len() as f32 * 4.6 + 12.;
                p.rect_filled(
                    Rect::from_center_size(pos2(rect.center().x, y), vec2(half * 2., 22.)),
                    0,
                    CARD,
                );
                text(pos2(rect.center().x, y), Align2::CENTER_CENTER, &label, 15., MUTED);
            }
        }
        text(pos2(l, t + 108.), Align2::LEFT_CENTER, &flight.from_city, 15., MUTED);
        text(pos2(r, t + 108.), Align2::RIGHT_CENTER, &flight.to_city, 15., MUTED);
        let clock = |s: &str| trip::moment(s).map(|m| m.clock).unwrap_or_default();
        text(pos2(l, t + 138.), Align2::LEFT_CENTER, &clock(&flight.departs), 24., TEXT);
        text(pos2(r, t + 138.), Align2::RIGHT_CENTER, &clock(&flight.arrives), 24., TEXT);
        text(pos2(l, t + 168.), Align2::LEFT_CENTER, &flight.from_terminal, 14., MUTED);
        text(pos2(r, t + 168.), Align2::RIGHT_CENTER, &flight.to_terminal, 14., MUTED);
        // The boarding pass sits in the middle of the bottom edge on every card.
        let slot = Rect::from_center_size(pos2(rect.center().x, t + 153.), vec2(190., 44.));
        if let Some(pass) = trip::boarding_pass(flight) {
            let button = Button::new(RichText::new("Boarding pass").color(BG).strong())
                .fill(self.accent)
                .corner_radius(8);
            if ui.put(slot, button).clicked() {
                if let Err(e) = std::process::Command::new("/usr/bin/xdg-open")
                    .arg(pass)
                    .spawn()
                {
                    self.notice = Some((e.to_string(), true, Instant::now()));
                }
            }
        } else if rect.width() > 380. {
            text(slot.center(), Align2::CENTER_CENTER, "No boarding pass saved", 14., MUTED);
        }
    }
    /// The trip at the top of the planner: every flight as a card, stops between them.
    fn trip_cards(&mut self, ui: &mut Ui, trip: &trip::Trip) {
        let full = ui.available_width() - 16.;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        let (first, last) = (&trip.flights[0], &trip.flights[trip.flights.len() - 1]);
        ui.horizontal(|ui| {
            ui.label(RichText::new("Your trip").size(29.).strong());
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add_space(16.);
                ui.label(
                    RichText::new(trip::status(trip, now))
                        .size(18.)
                        .color(self.accent)
                        .strong(),
                );
            });
        });
        let mut line = format!("{} to {}", first.from_city, last.to_city);
        if let Some(m) = trip::moment(&first.departs) {
            line += &format!("  ·  {}", m.day);
        }
        if !trip.booking.is_empty() {
            line += &format!("  ·  Booking {}", trip.booking);
        }
        ui.label(RichText::new(line).size(16.).color(MUTED));
        ui.add_space(14.);
        let (n, gap, stop_w, height) = (trip.flights.len() as f32, 14., 150., 196.);
        let card_w = (full - (n - 1.) * (stop_w + 2. * gap)) / n;
        if card_w < 330. {
            // Narrow window: one card per row, all the same width.
            for flight in &trip.flights {
                let (rect, _) = ui.allocate_exact_size(vec2(full, height), Sense::hover());
                self.flight_card(ui, rect, flight);
                ui.add_space(gap);
            }
            return;
        }
        let (row, _) = ui.allocate_exact_size(vec2(full, height), Sense::hover());
        let mut x = row.left();
        for (i, flight) in trip.flights.iter().enumerate() {
            let rect = Rect::from_min_size(pos2(x, row.top()), vec2(card_w, height));
            self.flight_card(ui, rect, flight);
            x += card_w + gap;
            if let Some(next) = trip.flights.get(i + 1) {
                let c = pos2(x + stop_w / 2., row.center().y);
                let p = ui.painter();
                p.text(c - vec2(0., 30.), Align2::CENTER_CENTER, "Stopover", FontId::proportional(14.), MUTED);
                p.text(c, Align2::CENTER_CENTER, &flight.to_city, FontId::proportional(19.), TEXT);
                if let Some(m) = trip::minutes_between(&flight.arrives, &next.departs) {
                    p.text(
                        c + vec2(0., 30.),
                        Align2::CENTER_CENTER,
                        trip::hours(m),
                        FontId::proportional(18.),
                        self.accent,
                    );
                }
                x += stop_w + gap;
            }
        }
    }
    fn flight_planner(&mut self, ui: &mut Ui) {
        let trip = trip::load();
        if let Some(trip) = &trip {
            self.trip_cards(ui, trip);
            ui.add_space(30.);
        }
        if self.history.plan.is_empty() {
            self.history.plan = vec![180];
        }
        let count = self.history.plan.len();
        self.history.plan_kinds.resize(count, "flight".into());
        let before = (self.history.plan.clone(), self.history.plan_kinds.clone());
        let names = planner::segment_names(&self.history.plan_kinds, count);
        let ordered = curation::home_items(
            &self.items,
            &self.progress,
            self.travel_enabled,
            self.show_unverified,
        );
        let remaining = ordered
            .iter()
            .map(|i| {
                (
                    i.id.clone(),
                    i.seconds.saturating_sub(self.progress.seconds(i) as u64),
                )
            })
            .collect();
        let plans = planner::trip(&ordered, &self.history.plan, &remaining);
        let full = ui.available_width() - 16.;
        ui.horizontal(|ui| {
            ui.label(RichText::new("Your line-up").size(29.).strong());
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add_space(16.);
                ui.label(RichText::new(self.battery.label()).size(16.).color(MUTED));
            });
        });
        ui.label(
            RichText::new("Set the time you can actually use. Films count their remaining runtime; reading and games are suggestions.")
                .size(16.)
                .color(MUTED),
        );
        self.nav_columns = 1;
        let legs = self.history.plan.clone();
        let kinds = self.history.plan_kinds.clone();
        let (mut adjust, mut remove, mut open, mut launch) = (None, None, None, None);
        let mut flight_index = 0;
        for (leg, plan) in plans.into_iter().enumerate() {
            // What this part of the trip is, from the saved flight details.
            let context = match &trip {
                Some(t) if kinds[leg] == "stop" => t
                    .flights
                    .get(flight_index.max(1) - 1)
                    .zip(t.flights.get(flight_index))
                    .map(|(a, b)| {
                        let wait = trip::minutes_between(&a.arrives, &b.departs)
                            .map(|m| format!("  ·  {} between flights", trip::hours(m)))
                            .unwrap_or_default();
                        format!("{}{wait}", a.to_city)
                    })
                    .unwrap_or_default(),
                Some(t) => t
                    .flights
                    .get(flight_index)
                    .map(|f| {
                        let air = trip::minutes_between(&f.departs, &f.arrives)
                            .map(|m| format!("  ·  {} gate to gate", trip::hours(m)))
                            .unwrap_or_default();
                        format!("{}  ·  {} to {}{air}", f.number, f.from_code, f.to_code)
                    })
                    .unwrap_or_default(),
                None => String::new(),
            };
            if kinds[leg] != "stop" {
                flight_index += 1;
            }
            let total: u32 = plan.iter().map(|s| s.minutes).sum();
            ui.add_space(22.);
            // Header: name and context on the left; time, summary and remove in fixed columns on the right.
            let (head, _) = ui.allocate_exact_size(vec2(full, 54.), Sense::hover());
            let y = head.center().y;
            let p = ui.painter().clone();
            p.text(
                head.left_center(),
                Align2::LEFT_CENTER,
                if count > 1 { names[leg].as_str() } else { "Flight" },
                FontId::proportional(22.),
                TEXT,
            );
            p.text(
                pos2(head.left() + 130., y),
                Align2::LEFT_CENTER,
                &context,
                FontId::proportional(16.),
                MUTED,
            );
            let right = head.right();
            let square = |x: f32| Rect::from_center_size(pos2(x, y), vec2(54., 50.));
            if count > 1
                && ui
                    .put(
                        Rect::from_center_size(pos2(right - 50., y), vec2(100., 50.)),
                        Button::new(RichText::new("Remove").size(16.)),
                    )
                    .clicked()
            {
                remove = Some(leg);
            }
            if ui.put(square(right - 144.), Button::new("+")).clicked() {
                adjust = Some((leg, 15_i32));
            }
            p.text(
                pos2(right - 230., y),
                Align2::CENTER_CENTER,
                trip::hours(legs[leg]),
                FontId::proportional(22.),
                TEXT,
            );
            if ui.put(square(right - 316.), Button::new("−")).clicked() {
                adjust = Some((leg, -15));
            }
            p.text(
                pos2(right - 364., y),
                Align2::RIGHT_CENTER,
                format!(
                    "{} planned  ·  {} min spare",
                    trip::hours(total),
                    legs[leg].saturating_sub(total)
                ),
                FontId::proportional(16.),
                self.accent,
            );
            for slot in plan {
                self.nav_cards.push(slot.item.id.clone());
                let focused = self.focus_ring && self.focused_card.as_ref() == Some(&slot.item.id);
                ui.add_space(8.);
                // Every row is the full width, with the same columns: picture, time, title, action.
                let (rect, response) = ui.allocate_exact_size(vec2(full, 76.), Sense::click());
                ui.painter().rect_filled(
                    rect,
                    12,
                    if response.hovered() { Color32::from_rgb(35, 44, 56) } else { CARD },
                );
                if focused {
                    ui.painter().rect_stroke(
                        rect,
                        12,
                        Stroke::new(3.0_f32, self.accent),
                        StrokeKind::Inside,
                    );
                    self.focused_leg = leg;
                }
                let thumb = Rect::from_min_size(rect.min + vec2(8., 8.), vec2(107., 60.));
                self.picture_fit(ui, thumb, &slot.item.image, false, slot.item.kind == "read");
                let cy = rect.center().y;
                ui.painter().text(
                    pos2(rect.left() + 134., cy),
                    Align2::LEFT_CENTER,
                    format!(
                        "{} min{}",
                        slot.minutes,
                        if slot.estimated { " suggested" } else { "" }
                    ),
                    FontId::proportional(18.),
                    self.accent,
                );
                let title = ui.painter().layout_no_wrap(
                    slot.item.title.clone(),
                    FontId::proportional(20.),
                    TEXT,
                );
                let title_at = pos2(rect.left() + 300., cy - title.size().y / 2.);
                ui.painter()
                    .with_clip_rect(Rect::from_min_max(
                        pos2(title_at.x, rect.top()),
                        pos2(rect.right() - 220., rect.bottom()),
                    ))
                    .galley(title_at, title, TEXT);
                let action = Rect::from_center_size(pos2(rect.right() - 108., cy), vec2(190., 50.));
                if ui.put(action, Button::new(action_label(&slot.item))).clicked() {
                    launch = Some(slot.item.clone());
                } else if response.clicked() {
                    open = Some(slot.item.id.clone());
                }
                if focused && self.scroll_focus {
                    ui.scroll_to_rect(rect, Some(Align::Center));
                    self.scroll_focus = false;
                }
            }
        }
        ui.add_space(22.);
        if count < 6 {
            ui.horizontal_wrapped(|ui| {
                if ui.button("Add a connecting flight").clicked() {
                    self.history.plan.push(90);
                    self.history.plan_kinds.push("flight".into());
                }
                if ui.button("Add a stopover").clicked() {
                    self.history.plan.push(60);
                    self.history.plan_kinds.push("stop".into());
                }
            });
        }
        if trip.is_none() {
            ui.label(
                RichText::new(format!(
                    "Flight details and boarding passes appear here when {} exists.",
                    trip::dir().join("trip.json").display()
                ))
                .size(14.)
                .color(MUTED),
            );
        }
        if let Some((leg, step)) = adjust {
            let minutes = &mut self.history.plan[leg];
            *minutes = (*minutes as i32 + step).clamp(15, 1440) as u32;
        }
        if let Some(leg) = remove {
            self.history.plan.remove(leg);
            self.history.plan_kinds.remove(leg);
        }
        if (self.history.plan.clone(), self.history.plan_kinds.clone()) != before {
            self.save();
        }
        if let Some(id) = open {
            self.detail = Some(id);
            self.gallery_index = 0;
        }
        if let Some(item) = launch {
            self.launch(&item);
        }
    }
    fn detail_page(&mut self, ui: &mut Ui, item: Item) {
        ui.horizontal(|ui| {
            if ui.button("Back to library").clicked() {
                self.detail = None;
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui
                    .button(if self.history.favorites.contains(&item.id) {
                        "★  Saved"
                    } else {
                        "☆  Save favorite"
                    })
                    .clicked()
                {
                    self.favorite(&item.id);
                }
            });
        });
        ui.add_space(16.);
        let wide = ui.available_width() > 980.;
        let left_width = if wide {
            ui.available_width() * 0.58
        } else {
            ui.available_width()
        };
        let selected = item
            .gallery
            .get(self.gallery_index)
            .unwrap_or(&item.image)
            .clone();
        let info = |this: &mut Self, ui: &mut Ui| {
            ui.label(
                RichText::new(item.tag.to_uppercase())
                    .color(this.accent)
                    .size(16.)
                    .strong(),
            );
            ui.label(RichText::new(&item.title).size(29.).strong());
            ui.label(RichText::new(&item.subtitle).color(MUTED));
            if item.kind == "watch" && item.subtitles {
                ui.label(RichText::new("Subtitles").color(this.accent).strong());
            }
            ui.add_space(10.);
            ui.label(
                RichText::new(format!("{}  ·  Available offline", item.meta))
                    .size(16.)
                    .color(Color32::from_rgb(128, 198, 176)),
            );
            if item.seconds > 0 && this.progress.seconds(&item) > 0. {
                let position = this.progress.seconds(&item);
                ui.add(
                    ProgressBar::new((position / item.seconds as f64) as f32)
                        .desired_height(12.)
                        .text(format!(
                            "{} min of {} min",
                            position as u64 / 60,
                            item.seconds / 60
                        )),
                );
            }
            ui.add_space(12.);
            ui.label(
                RichText::new(&item.description)
                    .size(20.)
                    .line_height(Some(28.)),
            );
            ui.add_space(18.);
            if ui
                .add_sized(
                    [ui.available_width(), 64.],
                    Button::new(RichText::new(action_label(&item)).strong().color(BG))
                        .fill(this.accent),
                )
                .clicked()
            {
                this.launch(&item);
            }
            ui.add_space(12.);
            match item.kind.as_str() {
                "listen" => {
                    ui.label(RichText::new("Plays in mpv without video. Quit normally to save your listening position.").size(16.).color(MUTED));
                }
                "watch" => {
                    ui.label(RichText::new("Opens in mpv. Space pauses; arrows seek. Quit normally to remember your place.").color(MUTED).size(16.));
                }
                "read" => {
                    ui.label(RichText::new(if item.path.ends_with(".html") {"Opens as a complete offline book in your browser, with large text and original illustrations."} else {"Opens in Document Viewer. Zoom, search, and browse the complete issue offline."}).color(MUTED).size(16.));
                }
                "atari" => {
                    ui.label(RichText::new("F11: fullscreen  ·  F12: settings / disks\nRight Alt + M: release mouse\nSaves use separate writable disk copies.").color(MUTED).size(16.));
                    ui.add_space(8.);
                    ui.label(
                        RichText::new(&item.notes)
                            .color(if item.verified {
                                MUTED
                            } else {
                                Color32::from_rgb(222, 180, 112)
                            })
                            .size(16.),
                    );
                }
                _ => {
                    ui.label(RichText::new("A new puzzle every time. Use the game's Help menu for rules and the Type menu for difficulty.").color(MUTED).size(16.));
                }
            }
            ui.add_space(16.);
            ui.separator();
            if item.bytes > 0 {
                ui.label(
                    RichText::new(format!(
                        "Local size  {:.1} MB",
                        item.bytes as f64 / 1_000_000.
                    ))
                    .size(16.)
                    .color(MUTED),
                );
            }
            ui.collapsing("File & source", |ui| {
                ui.label(RichText::new(&item.path).size(16.));
                ui.label(RichText::new(&item.source).size(16.).color(MUTED));
                if ui.small_button("Show in folder").clicked() {
                    if let Some(p) = std::path::Path::new(&item.path).parent() {
                        match std::process::Command::new("/usr/bin/xdg-open")
                            .arg(p)
                            .spawn()
                        {
                            Ok(_) => {}
                            Err(e) => this.notice = Some((e.to_string(), true, Instant::now())),
                        }
                    }
                }
            });
        };
        if wide {
            ui.horizontal_top(|ui| {
                ui.vertical(|ui| {
                    ui.set_width(left_width);
                    self.gallery(ui, &item, &selected, left_width);
                });
                ui.add_space(12.);
                ui.vertical(|ui| {
                    ui.set_width(ui.available_width());
                    info(self, ui);
                });
            });
        } else {
            info(self, ui);
            ui.add_space(22.);
            self.gallery(ui, &item, &selected, left_width);
        }
    }
    fn gallery(&mut self, ui: &mut Ui, item: &Item, selected: &str, width: f32) {
        let height = if item.kind == "read" {
            480.
        } else {
            width * 0.64
        };
        let (r, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
        self.picture(ui, r, selected, true);
        if item.gallery.len() > 1 {
            ui.add_space(7.);
            ui.horizontal_wrapped(|ui| {
                for (i, p) in item.gallery.iter().enumerate() {
                    let (r, response) = ui.allocate_exact_size(vec2(94., 62.), Sense::click());
                    self.picture(ui, r, p, true);
                    if i == self.gallery_index {
                        ui.painter().rect_stroke(
                            r,
                            4,
                            Stroke::new(2.0_f32, self.accent),
                            StrokeKind::Inside,
                        );
                    }
                    if response.clicked() {
                        self.gallery_index = i;
                    }
                }
            });
        }
        ui.add_space(6.);
        ui.label(
            RichText::new(match item.kind.as_str() {
                "watch" => "Film artwork and frames from your local copy",
                "read" => "Artwork from your offline reading",
                "puzzles" => "Screenshot from the puzzle author's website",
                _ => "Game screenshots / local boot captures",
            })
            .size(16.)
            .color(MUTED),
        );
    }
}
impl eframe::App for Cabin {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        self.frames += 1;
        if let Some(id) = self.open_on_start.take() {
            if let Some(item) = self.items.iter().find(|i| i.id == id).cloned() {
                self.launch(&item);
            }
        }
        let reading = self.reader_input(ctx);
        // One scripted move every quarter second, once the first cards are laid out.
        if !self.scripted_moves.is_empty() && self.detail.is_none() && !self.nav_cards.is_empty() {
            let due = 1500 + 250 * self.scripted_done as u128;
            if self.started.elapsed().as_millis() > due {
                let direction = self.scripted_moves.remove(0);
                self.scripted_done += 1;
                self.move_focus(direction);
            }
            ctx.request_repaint_after(Duration::from_millis(50));
        }
        for _ in 0..4 {
            match self.image_results.try_recv() {
                Ok((path, Some(image))) => {
                    let texture = ctx.load_texture(&path, image, TextureOptions::LINEAR);
                    self.textures.insert(path, texture);
                    ctx.request_repaint();
                }
                Ok((_, None)) => {}
                Err(_) => break,
            }
        }
        if !self.fullscreen_requested {
            ctx.request_repaint_after(Duration::from_millis(100));
            if self.started.elapsed() > Duration::from_millis(400) {
                if !desktop_fullscreen("set") {
                    ctx.send_viewport_cmd(ViewportCommand::Fullscreen(true));
                }
                self.fullscreen_requested = ctx.content_rect().width() >= 720.
                    || self.started.elapsed() > Duration::from_secs(4);
            }
        }
        if ctx.input(|i| i.key_pressed(Key::F11)) {
            let full = ctx.input(|i| i.viewport().fullscreen.unwrap_or(false));
            if !desktop_fullscreen("toggle") {
                ctx.send_viewport_cmd(ViewportCommand::Fullscreen(!full));
            }
        }
        if self.theme_check.elapsed() > Duration::from_secs(2) {
            self.accent = accent();
            self.theme_check = Instant::now();
            self.progress.refresh();
            self.battery = planner::Battery::read();
        }
        let events = ctx.input(|i| i.events.clone());
        for event in events {
            if let Event::Screenshot { image, .. } = event {
                if let Some(path) = self.screenshot.take() {
                    let bytes: Vec<u8> = image.pixels.iter().flat_map(|p| p.to_array()).collect();
                    if let Err(e) = image::save_buffer(
                        &path,
                        &bytes,
                        image.size[0] as u32,
                        image.size[1] as u32,
                        image::ColorType::Rgba8,
                    ) {
                        eprintln!("Screenshot failed: {e}");
                    }
                    ctx.send_viewport_cmd(ViewportCommand::Close);
                }
            }
        }
        if self.screenshot.is_some()
            && !self.capture_requested
            && self.started.elapsed() > Duration::from_secs(5)
        {
            self.capture_requested = true;
            ctx.send_viewport_cmd(ViewportCommand::Screenshot(Default::default()));
        }
        if self.screenshot.is_some() {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
        let controller_enabled =
            ctx.input(|i| i.viewport().focused.unwrap_or(false)) && self.children.is_empty();
        self.gamepad.set_enabled(controller_enabled);
        while let Ok(message) = self.gamepad.events.try_recv() {
            match message {
                gamepad::Message::Connected(name) => self.controller = name,
                gamepad::Message::Error(e) => eprintln!("Controller unavailable: {e}"),
                gamepad::Message::Action(action)
                    if controller_enabled && self.children.is_empty() =>
                {
                    self.controller_action(ctx, action)
                }
                _ => {}
            }
        }
        if ctx.input(|i| i.modifiers.command && i.key_pressed(Key::F)) {
            self.focus_search = true;
        }
        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            if self.help {
                self.help = false;
            } else if self.detail.is_some() {
                self.detail = None;
            } else {
                self.query.clear();
            }
        }
        if !ctx.wants_keyboard_input() {
            if ctx.input(|i| i.key_pressed(Key::Q)) {
                ctx.send_viewport_cmd(ViewportCommand::Close);
            }
            if ctx.input(|i| i.key_pressed(Key::Questionmark)) {
                self.help = !self.help;
            }
            if self.detail.is_none() {
                for (key, direction) in [
                    (Key::ArrowLeft, navigation::Direction::Left),
                    (Key::ArrowRight, navigation::Direction::Right),
                    (Key::ArrowUp, navigation::Direction::Up),
                    (Key::ArrowDown, navigation::Direction::Down),
                ] {
                    if ctx.input(|i| i.key_pressed(key)) {
                        self.move_focus(direction);
                    }
                }
                if ctx.input(|i| i.key_pressed(Key::Enter)) {
                    if let Some(id) = self
                        .focused_card
                        .clone()
                        .or_else(|| self.nav_cards.first().cloned())
                    {
                        self.detail = Some(id);
                        self.gallery_index = 0;
                    }
                }
            } else if ctx.input(|i| i.key_pressed(Key::Space) || i.key_pressed(Key::Enter)) {
                if let Some(item) = self
                    .items
                    .iter()
                    .find(|x| Some(&x.id) == self.detail.as_ref())
                    .cloned()
                {
                    self.launch(&item);
                }
            }
        }
        if let Some(child) = self.audio_scan.as_mut() {
            if let Ok(Some(status)) = child.try_wait() {
                self.audio_scan = None;
                match load() {
                    Ok(items) => {
                        self.progress = progress::Progress::new(&items);
                        self.items = items;
                    }
                    Err(e) => self.notice = Some((e, true, Instant::now())),
                }
                self.notice = Some((
                    if status.success() {
                        "Audio library refreshed".into()
                    } else {
                        "Some audio files could not be imported. See the launch log.".into()
                    },
                    !status.success(),
                    Instant::now(),
                ));
            } else {
                ctx.request_repaint_after(Duration::from_millis(200));
            }
        }
        self.nav_cards.clear();
        if ctx.input(|i| i.pointer.any_pressed()) {
            self.focus_ring = false;
        }
        let had_children = !self.children.is_empty();
        let mut errors = Vec::new();
        self.children
            .retain_mut(|(name, child)| match child.try_wait() {
                Ok(Some(status)) => {
                    if !status.success() {
                        errors.push(format!(
                            "{name} exited with an error. Details: {}",
                            state_dir().join("launch.log").display()
                        ));
                    }
                    false
                }
                Ok(None) => true,
                Err(e) => {
                    errors.push(e.to_string());
                    false
                }
            });
        if had_children && self.children.is_empty() {
            ctx.send_viewport_cmd(ViewportCommand::Focus);
            self.focus_ring = true;
        }
        for e in errors {
            self.notice = Some((e, true, Instant::now()));
        }
        if !self.children.is_empty() {
            ctx.request_repaint_after(Duration::from_secs(1));
        }
        if ctx.content_rect().width() < 320. || ctx.content_rect().height() < 360. {
            CentralPanel::default().show(ctx, |ui| {
                ui.add_space(30.);
                ui.heading("Cabin needs a little more room");
                ui.label("Open fullscreen to browse your offline collection.");
                if ui.button("Open fullscreen  ·  F11").clicked() {
                    ctx.send_viewport_cmd(ViewportCommand::Fullscreen(true));
                }
                if ui.button("Quit").clicked() {
                    ctx.send_viewport_cmd(ViewportCommand::Close);
                }
            });
            return;
        }
        self.sidebar(ctx);
        if self.touch_keyboard {
            TopBottomPanel::bottom("touch_keys")
                .frame(Frame::new().fill(PANEL).inner_margin(10))
                .show(ctx, |ui| {
                    let key_width = ((ui.available_width() - 9. * 6.) / 10.).min(70.);
                    ui.spacing_mut().item_spacing = vec2(6., 6.);
                    let mut index = 0;
                    for row in ["qwertyuiop", "asdfghjkl", "zxcvbnm"] {
                        ui.horizontal(|ui| {
                            for key in row.chars() {
                                let selected = self.focus_ring
                                    && self.controller.is_some()
                                    && self.keyboard_key == index;
                                if ui
                                    .add_sized(
                                        [key_width, 48.],
                                        Button::new(key.to_string()).stroke(Stroke::new(
                                            2.0_f32,
                                            if selected { self.accent } else { BORDER },
                                        )),
                                    )
                                    .clicked()
                                {
                                    self.keyboard_press(index);
                                }
                                index += 1;
                            }
                        });
                    }
                    ui.horizontal_wrapped(|ui| {
                        for (offset, label) in
                            ["Clear", "Space", "Delete", "Done"].iter().enumerate()
                        {
                            let index = 26 + offset;
                            let selected = self.focus_ring
                                && self.controller.is_some()
                                && self.keyboard_key == index;
                            if ui
                                .add_sized(
                                    [100., 48.],
                                    Button::new(*label).stroke(Stroke::new(
                                        2.0_f32,
                                        if selected { self.accent } else { BORDER },
                                    )),
                                )
                                .clicked()
                            {
                                self.keyboard_press(index);
                            }
                        }
                    });
                });
        }
        CentralPanel::default().frame(Frame::new().fill(BG).inner_margin(Margin::symmetric(30,24))).show(ctx,|ui|{
            if reading && self.reader.is_some() {
                self.reader_view(ui);
                return;
            }
            self.search_toolbar(ui);
            ui.add_space(24.);
            if let Some((text,error,_))=&self.notice{let t=text.clone();let error=*error;Frame::new().fill(if error{Color32::from_rgb(68,37,36)}else{Color32::from_rgb(26,53,48)}).corner_radius(8).inner_margin(10).show(ui,|ui|{ui.horizontal_wrapped(|ui|{ui.label(&t);if ui.small_button("Dismiss").clicked(){self.notice=None;}});});ui.add_space(10.);}
            ScrollArea::vertical().id_salt(("content",self.section.clone(),self.detail.clone())).auto_shrink([false,false]).show(ui,|ui|{
                if self.controller_scroll != 0. {ui.scroll_with_delta(vec2(0.,self.controller_scroll));self.controller_scroll=0.;}
                if let Some(item)=self.items.iter().find(|i|Some(&i.id)==self.detail.as_ref()).cloned(){self.detail_page(ui,item);return;}
                if self.section=="atari" {ui.checkbox(&mut self.show_unverified,"Show unverified games");ui.label(RichText::new("Verified means a game screen was reached; it is not a complete playthrough.").size(16.).color(MUTED));ui.add_space(12.);}
                if self.section=="listen" {
                    ui.label("Music, podcasts and audiobooks from ~/music/flight/");
                    if ui.add_enabled(self.audio_scan.is_none(),Button::new(if self.audio_scan.is_some(){"Scanning…"}else{"Scan audio folder"})).clicked(){
                        let log=state_dir().join("launch.log");let _=fs::create_dir_all(state_dir());
                        match fs::OpenOptions::new().create(true).append(true).open(log){
                            Ok(file)=>{let mut command=std::process::Command::new("python3");command.arg(home().join(".local/share/cabin/audio_library.py")).stdout(Stdio::from(file.try_clone().unwrap())).stderr(Stdio::from(file));match command.spawn(){Ok(child)=>self.audio_scan=Some(child),Err(e)=>self.notice=Some((e.to_string(),true,Instant::now()))}},
                            Err(e)=>self.notice=Some((e.to_string(),true,Instant::now())),
                        }
                    }ui.add_space(12.);
                }
                if self.section=="planner" {self.flight_planner(ui);return;}
                let (title,sub)=match self.section.as_str(){
                    "listen"=>("Let the world drift by.","Your local audio collection, ready to resume."),
                    "watch"=>("A window into another world.","Space, history, and the stories behind extraordinary events."),
                    "puzzles"=>("Just one more puzzle.","Forty little challenges. No accounts, no timers, no connection needed."),
                    "atari"=>("Back to the good stuff.","Your Atari ST collection, with the familiar games and a few new discoveries."),
                    "read"=>("The long way through a page.","Roman life, Swedish history, space and retro computing. Saved locally."),
                    "travel"=>("Somewhere you haven’t been.","Seas, islands, beaches and mountains. The Americas, Asia and the Caribbean."),
                    "favorites"=>("Your personal shortlist.","The things you have saved for later."),
                    "recent"=>("Pick up where you left off.","Recently opened. Videos remember their playback position."),
                    _=>("Settle in. Switch off.","A good flight starts with something worth getting lost in."),};
                ui.label(RichText::new(title).size(31.).strong());ui.label(RichText::new(sub).color(MUTED).size(16.));ui.add_space(20.);
                if self.section=="home"&&self.query.is_empty(){
                    let continuing=self.progress.continuing(&self.items);
                    if !continuing.is_empty(){ui.heading("Continue watching");self.grid(ui,continuing);ui.add_space(22.);}
                    let ordered=curation::home_items(&self.items,&self.progress,self.travel_enabled,self.show_unverified);
                    if ui.available_width()>850. {if let Some(item)=ordered.first(){self.hero(ui,item.clone());ui.add_space(21.);}}
                    let travel:Vec<_>=ordered.iter().filter(|i|self.travel_enabled && i.tag=="Travel" && !self.progress.unfinished(i)).cloned().collect();
                    if !travel.is_empty(){ui.heading("Your Travel shelf");self.grid(ui,travel);ui.add_space(22.);}
                    ui.horizontal_wrapped(|ui|{ui.label(RichText::new("Packed for the journey").size(21.).strong());ui.with_layout(Layout::right_to_left(Align::Center),|ui|{ui.label(RichText::new(format!("{} films  ·  {} puzzles  ·  {} reads",self.items.iter().filter(|i|i.kind=="watch").count(),self.items.iter().filter(|i|i.kind=="puzzles").count(),self.items.iter().filter(|i|i.kind=="read").count())).size(16.).color(MUTED));});});ui.add_space(9.);
                    let items=ordered.into_iter().filter(|i|!self.progress.unfinished(i) && !(self.travel_enabled && i.tag=="Travel")).take(12).collect();self.grid(ui,items);
                }else{
                    let mut items:Vec<Item>=self.items.iter().filter(|i|visible_game(i,self.show_unverified)&&matches(i,&self.query)&&match self.section.as_str(){"home"=>true,"travel"=>i.tag=="Travel","favorites"=>self.history.favorites.contains(&i.id),"recent"=>self.history.recent.contains(&i.id),s=>i.kind==s}).cloned().collect();
                    if self.section=="recent"{items.sort_by_key(|i|self.history.recent.iter().position(|id|id==&i.id).unwrap_or(999));}
                    ui.label(RichText::new(format!("{} items · stored on this laptop",items.len())).color(MUTED).size(16.));ui.add_space(8.);self.grid(ui,items);
                }
            });
        });
        if self.help {
            Window::new("Make yourself at home")
                .collapsible(false)
                .resizable(false)
                .anchor(Align2::CENTER_CENTER, [0., 0.])
                .open(&mut self.help)
                .show(ctx, |ui| {
                    for (k, v) in [
                        ("Ctrl + F", "Search the collection"),
                        ("Escape", "Back / clear search"),
                        ("Arrows", "Move through the grid"),
                        ("Enter", "Open details / launch"),
                        ("Space", "Launch from details"),
                        ("Q", "Quit Cabin"),
                        ("F11", "Toggle fullscreen"),
                        ("F12 in Hatari", "Settings and disk swaps"),
                        ("Right Alt + M", "Release the game's mouse"),
                    ] {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(k).strong());
                            ui.label(v);
                        });
                    }
                });
        }
    }
}
fn arg_value(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|p| args.get(p + 1))
        .cloned()
}
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args
        .first()
        .is_some_and(|x| x == "check" || x == "list" || x == "launch")
    {
        let result = (|| -> Result<(), String> {
            let items = load()?;
            match args[0].as_str() {
                "list" => println!("{}", serde_json::to_string_pretty(&items).unwrap()),
                "check" => {
                    let report = validation::check(&items);
                    println!("{}", serde_json::to_string(&report).unwrap());
                    if !report.errors.is_empty() {
                        return Err("Library check failed".into());
                    }
                }

                "launch" => {
                    let id = args
                        .get(1)
                        .ok_or("Usage: cabin launch ITEM_ID [--dry-run]")?;
                    let item = items
                        .iter()
                        .find(|i| &i.id == id)
                        .ok_or("Unknown item id")?;
                    if args.iter().any(|a| a == "--dry-run") {
                        let (exe, a) = command_args(item, false)?;
                        println!("{}", serde_json::json!({"executable":exe,"args":a}));
                    } else {
                        build_command(item)?.spawn().map_err(|e| e.to_string())?;
                    }
                }
                _ => {}
            }
            Ok(())
        })();
        if let Err(e) = result {
            eprintln!("{e}");
            std::process::exit(1);
        }
        return;
    }
    let known = [
        "home", "planner", "travel", "watch", "listen", "puzzles", "atari", "read", "favorites", "recent",
    ];
    let section = arg_value(&args, "--section")
        .map(|s| if s == "flight" { "planner".to_string() } else { s })
        .filter(|s| known.contains(&s.as_str()))
        .unwrap_or("home".into());
    let detail = arg_value(&args, "--detail");
    let screenshot = arg_value(&args, "--screenshot").map(PathBuf::from);
    let read = arg_value(&args, "--read");
    let moves: Vec<navigation::Direction> = arg_value(&args, "--moves")
        .unwrap_or_default()
        .split(',')
        .filter_map(|m| match m.trim() {
            "left" => Some(navigation::Direction::Left),
            "right" => Some(navigation::Direction::Right),
            "up" => Some(navigation::Direction::Up),
            "down" => Some(navigation::Direction::Down),
            _ => None,
        })
        .collect();
    let options = eframe::NativeOptions {
        viewport: ViewportBuilder::default()
            .with_title("Cabin — your offline escape")
            .with_inner_size([1220., 820.])
            .with_min_inner_size([360., 400.])
            .with_app_id("cabin"),
        renderer: eframe::Renderer::Glow,
        event_loop_builder: Some(Box::new(|builder| {
            if std::env::args().any(|a| a == "--wayland") {
                builder.with_wayland();
            } else {
                builder.with_x11();
            }
        })),
        ..Default::default()
    };
    if let Err(e) = eframe::run_native(
        "cabin",
        options,
        Box::new(move |cc| Ok(Box::new(Cabin::new(cc, section, detail, screenshot, moves, read)))),
    ) {
        eprintln!("Could not open Cabin: {e}");
        std::process::exit(1);
    }
}
