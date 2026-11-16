mod curation;
mod library;
mod navigation;
mod progress;
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
    gallery_index: usize,
    accent: Color32,
    notice: Option<(String, bool, Instant)>,
    children: Vec<(String, Child)>,
    help: bool,
    touch_keyboard: bool,
    show_unverified: bool,
    travel_enabled: bool,
    fullscreen_requested: bool,
    started: Instant,
    capture_requested: bool,
    focus_search: bool,
    screenshot: Option<PathBuf>,
    frames: u32,
    theme_check: Instant,
}
impl Cabin {
    fn new(
        cc: &eframe::CreationContext<'_>,
        section: String,
        detail: Option<String>,
        screenshot: Option<PathBuf>,
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
            gallery_index: 0,
            accent: accent(),
            notice,
            children: vec![],
            help: false,
            show_unverified: false,
            travel_enabled: curation::travel_enabled(),
            touch_keyboard: std::env::args().any(|a| a == "--keyboard"),
            fullscreen_requested: std::env::args().any(|a| a == "--windowed"),
            started: Instant::now(),
            capture_requested: false,
            focus_search: false,
            screenshot,
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

    fn picture(&mut self, ui: &Ui, rect: Rect, path: &str, contain: bool) {
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
                    Rect::from_min_max(pos2(0., (1. - f) / 2.), pos2(1., (1. + f) / 2.))
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
    fn launch(&mut self, item: &Item) {
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
    fn sidebar(&mut self, ctx: &Context) {
        TopBottomPanel::bottom("touch_navigation")
            .frame(Frame::new().fill(PANEL).inner_margin(12))
            .show(ctx, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = vec2(8., 8.);
                    for (id, label) in [
                        ("home", "Discover"),
                        ("travel", "Travel"),
                        ("watch", "Watch"),
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
        self.picture(ui, image_rect, &item.image, item.kind != "watch");
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
        self.picture(ui, pic, &item.image, false);
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
                        let current = self
                            .focused_card
                            .as_ref()
                            .and_then(|id| self.nav_cards.iter().position(|x| x == id));
                        if let Some(next) = navigation::next_index(
                            current,
                            self.nav_cards.len(),
                            self.nav_columns,
                            direction,
                        ) {
                            self.focused_card = Some(self.nav_cards[next].clone());
                            self.focus_ring = true;
                            self.scroll_focus = true;
                        }
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
        self.nav_cards.clear();
        if ctx.input(|i| i.pointer.any_pressed()) {
            self.focus_ring = false;
        }
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
                    for row in ["qwertyuiop", "asdfghjkl", "zxcvbnm"] {
                        ui.horizontal(|ui| {
                            for key in row.chars() {
                                if ui
                                    .add_sized([key_width, 48.], Button::new(key.to_string()))
                                    .clicked()
                                {
                                    self.query.push(key);
                                    self.detail = None;
                                }
                            }
                        });
                    }
                    ui.horizontal(|ui| {
                        if ui.add_sized([100., 48.], Button::new("Clear")).clicked() {
                            self.query.clear();
                        }
                        if ui
                            .add_sized([key_width * 3., 48.], Button::new("Space"))
                            .clicked()
                        {
                            self.query.push(' ');
                        }
                        if ui.add_sized([100., 48.], Button::new("Delete")).clicked() {
                            self.query.pop();
                        }
                        if ui.add_sized([100., 48.], Button::new("Done")).clicked() {
                            self.touch_keyboard = false;
                        }
                    });
                });
        }
        CentralPanel::default().frame(Frame::new().fill(BG).inner_margin(Margin::symmetric(30,24))).show(ctx,|ui|{
            ui.horizontal(|ui|{
                if ui.available_width() > 650. {ui.label(RichText::new("CABIN / OFFLINE").size(20.).color(MUTED).strong());}
                ui.with_layout(Layout::right_to_left(Align::Center),|ui|{
                    if ui.button("Keys").clicked(){self.touch_keyboard=!self.touch_keyboard;}
                    if ui.button("Reload").on_hover_text("Reload local library").clicked(){match load(){Ok(i)=>{self.progress=progress::Progress::new(&i);self.travel_enabled=curation::travel_enabled();self.items=i;},Err(e)=>self.notice=Some((e,true,Instant::now()))}}
                    let r=ui.add_sized([ui.available_width().min(380.),54.],TextEdit::singleline(&mut self.query).hint_text("Search your collection…").id(Id::new("search")));
                    if self.focus_search{r.request_focus();self.focus_search=false;}
                });
            });ui.add_space(18.);
            if let Some((text,error,_))=&self.notice{let t=text.clone();let error=*error;Frame::new().fill(if error{Color32::from_rgb(68,37,36)}else{Color32::from_rgb(26,53,48)}).corner_radius(8).inner_margin(10).show(ui,|ui|{ui.horizontal_wrapped(|ui|{ui.label(&t);if ui.small_button("Dismiss").clicked(){self.notice=None;}});});ui.add_space(10.);}
            ScrollArea::vertical().id_salt(("content",self.section.clone(),self.detail.clone())).auto_shrink([false,false]).show(ui,|ui|{
                if let Some(item)=self.items.iter().find(|i|Some(&i.id)==self.detail.as_ref()).cloned(){self.detail_page(ui,item);return;}
                if self.section=="atari" {ui.checkbox(&mut self.show_unverified,"Show unverified games");ui.label(RichText::new("Verified means a game screen was reached; it is not a complete playthrough.").size(16.).color(MUTED));ui.add_space(12.);}
                let (title,sub)=match self.section.as_str(){
                    "watch"=>("A window into another world.","Space, history, and the stories behind extraordinary events."),
                    "puzzles"=>("Just one more puzzle.","Forty little challenges. No accounts, no timers, no connection needed."),
                    "atari"=>("Back to the good stuff.","Your Atari ST collection, with the familiar games and a few new discoveries."),
                    "read"=>("The long way through a page.","Islands, sailing, space and retro computing. Complete issues, saved locally."),
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
                    ui.horizontal_wrapped(|ui|{ui.label(RichText::new("Packed for the journey").size(21.).strong());ui.with_layout(Layout::right_to_left(Align::Center),|ui|{ui.label(RichText::new(format!("{} films  ·  {} puzzles  ·  {} magazines",self.items.iter().filter(|i|i.kind=="watch").count(),self.items.iter().filter(|i|i.kind=="puzzles").count(),self.items.iter().filter(|i|i.kind=="read").count())).size(16.).color(MUTED));});});ui.add_space(9.);
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
    let section = arg_value(&args, "--section").unwrap_or("home".into());
    let detail = arg_value(&args, "--detail");
    let screenshot = arg_value(&args, "--screenshot").map(PathBuf::from);
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
        Box::new(move |cc| Ok(Box::new(Cabin::new(cc, section, detail, screenshot)))),
    ) {
        eprintln!("Could not open Cabin: {e}");
        std::process::exit(1);
    }
}
