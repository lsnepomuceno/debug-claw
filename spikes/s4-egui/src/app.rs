//! The app shell (top bar, rail, active view), lazy dataset loading and
//! the automation modes: benchmark, screenshot and quit-after.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use eframe::egui::{
    self, Align, Frame, Layout, Margin, Rect, RichText, Stroke, ThemePreference, Ui, vec2,
};
use egui_phosphor::regular as ph;

use crate::actions::{self, ActionId, Style};
use crate::alloc;
use crate::bench::{Bench, Scenario};
use crate::dumps::Feed;
use crate::flame::{self, Profile};
use crate::mockups::{self, Debugger, FeedState};
use crate::source::{self, Source};
use crate::theme::{self, size};
use crate::tree::{self, VarTree};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum View {
    Feed,
    Requests,
    Debugger,
    Flame,
    Breakpoints,
    Settings,
    Tree,
    Source,
}

impl View {
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "feed" | "dumps" => Self::Feed,
            "debugger" => Self::Debugger,
            "flame" | "profiler" => Self::Flame,
            "tree" => Self::Tree,
            "source" => Self::Source,
            _ => return None,
        })
    }
}

pub struct Opts {
    pub view: View,
    pub theme: Option<ThemePreference>,
    pub backend: &'static str,
    pub tree_nodes: usize,
    pub dumps: usize,
    pub flame_frames: usize,
    pub source_lines: usize,
    pub expand_all: bool,
    pub expand_sample: usize,
    pub bench: Option<(Scenario, usize)>,
    pub screenshot: Option<PathBuf>,
    pub hover: Option<ActionId>,
    /// Seconds before the screenshot, so a script can send input first.
    pub screenshot_delay: Option<f32>,
    pub quit_after: Option<usize>,
    pub load_all: bool,
    pub live_countdown: bool,
}

static RECORD_RECTS: AtomicBool = AtomicBool::new(false);

fn rect_id(id: ActionId) -> egui::Id {
    egui::Id::unique(("action-rect", id as u8))
}

/// Remember where an action's button was drawn, so screenshot mode can
/// put the pointer on it. Off unless a hover target was given.
pub fn record_rect(ctx: &egui::Context, id: ActionId, rect: Rect) {
    if RECORD_RECTS.load(Ordering::Relaxed) {
        ctx.data_mut(|d| d.insert_temp(rect_id(id), rect));
    }
}

pub struct App {
    pub opts: Opts,
    view: View,
    feed: Option<FeedState>,
    dbg: Option<Debugger>,
    tree: Option<VarTree>,
    flame: Option<Profile>,
    source: Option<Source>,
    bench: Option<Bench>,
    last_cpu: Option<f32>,
    content: Rect,
    frame_no: usize,
    reported: bool,
    /// S4_LOG_FRAMES set: print one line per frame to stderr, so the idle
    /// scripts can count repaints.
    log_frames: bool,
    started: Instant,
    shot_requested: bool,
    shot_done: Arc<AtomicBool>,
    pub log: Vec<String>,
}

impl App {
    pub fn new(opts: Opts) -> Self {
        RECORD_RECTS.store(opts.hover.is_some(), Ordering::Relaxed);
        let bench = opts.bench.map(|(s, n)| Bench::new(s, n));
        let mut app = App {
            view: opts.view,
            opts,
            feed: None,
            dbg: None,
            tree: None,
            flame: None,
            source: None,
            bench,
            last_cpu: None,
            content: Rect::from_min_size(egui::pos2(400.0, 300.0), vec2(800.0, 500.0)),
            frame_no: 0,
            reported: false,
            log_frames: std::env::var_os("S4_LOG_FRAMES").is_some(),
            started: Instant::now(),
            shot_requested: false,
            shot_done: Arc::new(AtomicBool::new(false)),
            log: Vec::new(),
        };
        if app.opts.load_all {
            for v in [View::Feed, View::Debugger, View::Tree, View::Flame, View::Source] {
                app.ensure_loaded(v);
            }
        } else {
            app.ensure_loaded(app.view);
        }
        app
    }

    /// Build a view's data the first time it is shown.
    fn ensure_loaded(&mut self, v: View) {
        let t = Instant::now();
        let what = match v {
            View::Feed if self.feed.is_none() => {
                let curated = self.opts.dumps <= 1000;
                self.feed = Some(FeedState { feed: Feed::new(self.opts.dumps, curated), paused: false, clear_on_request: false, filter: String::new() });
                format!("feed dumps={}", self.opts.dumps)
            }
            View::Debugger if self.dbg.is_none() => {
                self.dbg = Some(Debugger::new(self.opts.live_countdown));
                "debugger".into()
            }
            View::Tree if self.tree.is_none() => {
                let mut tr = VarTree::generated(self.opts.tree_nodes);
                let mut extra = String::new();
                if self.opts.expand_all {
                    let t2 = Instant::now();
                    tr.expand_all();
                    extra = format!(" expand_all_ms={:.1}", t2.elapsed().as_secs_f64() * 1000.0);
                } else if self.opts.expand_sample > 0 {
                    tr.expand_sample(self.opts.expand_sample);
                } else if matches!(self.bench.as_ref().map(|b| b.scenario), Some(Scenario::TreeExpand | Scenario::TreeScroll)) {
                    tr.toggle_row(0);
                }
                let s = format!("tree virtual_nodes={} fetched={} rows={}{extra}", tr.virtual_nodes, tr.fetched(), tr.rows.len());
                self.tree = Some(tr);
                s
            }
            View::Flame if self.flame.is_none() => {
                let p = Profile::generated(self.opts.flame_frames);
                let s = format!("flame frames={} max_depth={} names={}", p.frames.len(), p.max_depth, p.names.len());
                self.flame = Some(p);
                s
            }
            View::Source if self.source.is_none() => {
                let s = Source::generated(self.opts.source_lines);
                let m = format!("source lines={} highlight_ms={:.1}", s.lines(), s.highlight_ms);
                self.source = Some(s);
                m
            }
            _ => return,
        };
        self.log.push(format!("load {what} ms={:.1}", t.elapsed().as_secs_f64() * 1000.0));
    }

    fn top_bar(&mut self, ui: &mut Ui) {
        let p = theme::palette(ui.ctx());
        egui::Panel::top("top-bar")
            .exact_size(46.0)
            .frame(Frame::new().fill(p.surface).inner_margin(Margin::symmetric(12, 0)).stroke(Stroke::new(1.0, p.border)))
            .show(ui, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.label(RichText::new(ph::BUG).font(theme::icon_font(18.0)).color(p.accent));
                    ui.label(RichText::new("debug-claw").font(theme::semibold(size::HEADING)));
                    ui.add_space(14.0);
                    let (dot, _) = ui.allocate_exact_size(vec2(10.0, 10.0), egui::Sense::hover());
                    ui.painter().circle_filled(dot.center(), 4.0, p.success);
                    ui.label("Listening on 9003").on_hover_text("The DBGp listener accepts Xdebug connections on port 9003");
                    ui.label(RichText::new("|").color(p.border));
                    ui.label("2 sessions");
                    ui.label(RichText::new("|").color(p.border));
                    ui.label(theme::icon_job(ph::ROBOT, "Agent: idle", theme::body(size::BODY), p.text_weak));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if actions::button(ui, ActionId::ToggleTheme, Style::Icon, None).clicked() {
                            let next = if p.dark { ThemePreference::Light } else { ThemePreference::Dark };
                            ui.ctx().set_theme(next);
                        }
                        actions::button(ui, ActionId::AlwaysOnTop, Style::Icon, None);
                        actions::button(ui, ActionId::Search, Style::Labeled, None);
                    });
                });
            });
    }

    fn rail(&mut self, ui: &mut Ui) {
        let p = theme::palette(ui.ctx());
        let feed_badge = self.feed.as_ref().map(|f| f.feed.dumps.len().to_string());
        egui::Panel::left("rail")
            .exact_size(172.0)
            .resizable(false)
            .frame(Frame::new().fill(p.surface_alt).inner_margin(Margin::symmetric(8, 10)).stroke(Stroke::new(1.0, p.border)))
            .show(ui, |ui| {
                let items: [(View, &str, &str, Option<&str>); 6] = [
                    (View::Feed, ph::ROWS, "Feed", feed_badge.as_deref()),
                    (View::Requests, ph::LIST, "Requests", Some("38")),
                    (View::Debugger, ph::BUG, "Debugger", None),
                    (View::Flame, ph::FLAME, "Profiler", None),
                    (View::Breakpoints, ph::STACK, "Breakpoints", Some("2")),
                    (View::Settings, ph::GEAR, "Settings", None),
                ];
                for (v, icon, label, badge) in items {
                    if actions::nav_item(ui, icon, label, self.view == v, badge).clicked() {
                        self.view = v;
                    }
                }
                ui.add_space(14.0);
                ui.label(RichText::new("SPIKE DATA").font(theme::semibold(11.0)).color(p.text_weak));
                ui.add_space(2.0);
                for (v, icon, label) in [(View::Tree, ph::TREE_STRUCTURE, "Variable tree"), (View::Source, ph::FILE_CODE, "Large source")] {
                    if actions::nav_item(ui, icon, label, self.view == v, None).clicked() {
                        self.view = v;
                    }
                }
            });
    }

    fn header(ui: &mut Ui, title: &str, detail: &str) {
        let p = theme::palette(ui.ctx());
        ui.label(RichText::new(title).font(theme::semibold(size::HEADING)));
        ui.label(RichText::new(detail).color(p.text_weak).font(theme::body(size::SMALL)));
    }

    /// Everything inside the window. Separate from `eframe::App::ui` so the
    /// headless AccessKit dump can run it too.
    pub fn draw(&mut self, ui: &mut Ui) {
        self.ensure_loaded(self.view);
        let p = theme::palette(ui.ctx());
        self.top_bar(ui);
        self.rail(ui);
        let bench = self.bench.as_ref().map(|b| (b.scenario, b.frame));
        egui::CentralPanel::no_frame().frame(Frame::new().fill(p.bg)).show(ui, |ui| {
            self.content = ui.max_rect();
            match self.view {
                View::Feed => mockups::feed_view(ui, self.feed.as_mut().unwrap()),
                View::Debugger => mockups::debugger_view(ui, self.dbg.as_mut().unwrap()),
                View::Tree => {
                    let tr = self.tree.as_mut().unwrap();
                    if let Some((Scenario::TreeExpand, k)) = bench
                        && k > 3
                    {
                        // Expand one order on even frames, collapse it on odd ones.
                        tr.toggle_row(1 + (k / 2) % 50);
                    }
                    Frame::new().inner_margin(Margin::same(14)).show(ui, |ui| {
                        ui.horizontal(|ui| {
                            Self::header(ui, "Variable tree", &format!("{} nodes, {} fetched, {} rows visible", tr.virtual_nodes, tr.fetched(), tr.rows.len()));
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if ui.button("Collapse all").clicked() {
                                    tr.collapse_all();
                                }
                                if ui.button("Expand all").on_hover_text("Fetches every node: the worst case for memory").clicked() {
                                    tr.expand_all();
                                }
                            });
                        });
                        ui.add_space(6.0);
                        Frame::new().fill(p.surface).stroke(Stroke::new(1.0, p.border)).corner_radius(size::RADIUS).inner_margin(Margin::same(4)).show(ui, |ui| {
                            tree::show(ui, tr, "big-tree");
                        });
                    });
                }
                View::Flame => {
                    let prof = self.flame.as_mut().unwrap();
                    Frame::new().inner_margin(Margin::same(14)).show(ui, |ui| {
                        ui.horizontal(|ui| {
                            Self::header(ui, "Profile: POST /orders", &format!("{} frames, {} drawn, showing {:.3}% of the time", prof.frames.len(), prof.drawn_last_frame, prof.view_fraction() * 100.0));
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if ui.button("Reset zoom").clicked() {
                                    prof.reset();
                                }
                                ui.label(RichText::new("Ctrl+wheel zooms, drag pans, double click zooms to a frame").color(p.text_weak).font(theme::body(size::SMALL)));
                            });
                        });
                        ui.add_space(6.0);
                        let h = ui.available_height() - 30.0;
                        ui.allocate_ui(vec2(ui.available_width(), h), |ui| flame::show(ui, prof));
                        ui.add_space(6.0);
                        ui.label(RichText::new(flame::status(prof)).font(theme::mono(size::SMALL)));
                    });
                }
                View::Source => {
                    let src = self.source.as_mut().unwrap();
                    Frame::new().inner_margin(Margin::same(14)).show(ui, |ui| {
                        ui.horizontal(|ui| {
                            Self::header(ui, &src.name, &format!("{} lines, highlighted in {:.1} ms at load", src.lines(), src.highlight_ms));
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if ui.button("Go to line   Ctrl+G").clicked() {
                                    src.open_goto();
                                }
                            });
                        });
                        ui.add_space(6.0);
                        Frame::new().fill(p.surface).stroke(Stroke::new(1.0, p.border)).corner_radius(size::RADIUS).inner_margin(Margin::same(4)).show(ui, |ui| {
                            source::show(ui, src, "big-source");
                        });
                    });
                }
                View::Requests | View::Breakpoints | View::Settings => {
                    ui.add_space(120.0);
                    ui.vertical_centered(|ui| {
                        ui.label(RichText::new(ph::PROHIBIT).font(theme::icon_font(32.0)).color(p.text_weak));
                        ui.label(RichText::new("This view is not part of the S4 spike").color(p.text_weak));
                    });
                }
            }
        });
    }

    pub fn report(&self) -> String {
        format!(
            "heap view={:?} backend={} live_mb={:.1} peak_mb={:.1} rss_mb={:.1} | {}",
            self.view,
            self.opts.backend,
            alloc::mb(alloc::live()),
            alloc::mb(alloc::peak()),
            alloc::mb(alloc::rss()),
            self.log.join(" | ")
        )
    }

    fn automation(&mut self, ctx: &egui::Context) {
        self.frame_no += 1;
        if self.log_frames {
            eprintln!("frame {} t={:.3}", self.frame_no, self.started.elapsed().as_secs_f64());
        }
        if let Some(b) = &self.bench {
            if b.done() {
                if !self.reported {
                    self.reported = true;
                    let extra = format!("view={:?} rss_mb={:.1}", self.view, alloc::mb(alloc::rss()));
                    println!("{}", b.report(self.opts.backend, &extra));
                    println!("{}", self.report());
                }
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            } else {
                ctx.request_repaint();
            }
        }
        if let Some(n) = self.opts.quit_after {
            if self.frame_no >= n {
                // Close takes a frame or two to land; print once.
                if self.frame_no == n {
                    println!("{}", self.report());
                }
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            } else {
                ctx.request_repaint();
            }
        }
        if let Some(path) = self.opts.screenshot.clone() {
            if self.shot_done.load(Ordering::Relaxed) {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                return;
            }
            ctx.request_repaint();
            let wait = self.opts.screenshot_delay.unwrap_or(if self.opts.hover.is_some() { 2.0 } else { 1.0 });
            if !self.shot_requested && self.frame_no > 10 && self.started.elapsed().as_secs_f32() > wait {
                self.shot_requested = true;
                let done = self.shot_done.clone();
                ctx.request_screenshot(move |img| {
                    let [w, h] = img.size;
                    let rgb: Vec<u8> = img.pixels.iter().flat_map(|c| [c.r(), c.g(), c.b()]).collect();
                    match image::save_buffer(&path, &rgb, w as u32, h as u32, image::ColorType::Rgb8) {
                        Ok(()) => println!("screenshot {} {w}x{h}", path.display()),
                        Err(e) => eprintln!("screenshot failed: {e}"),
                    }
                    done.store(true, Ordering::Relaxed);
                });
            }
        }
    }
}

impl eframe::App for App {
    fn raw_input_hook(&mut self, ctx: &egui::Context, raw: &mut egui::RawInput) {
        let center = self.content.center();
        if let Some(b) = &mut self.bench {
            b.tick(self.last_cpu, raw, center);
        }
        if let Some(id) = self.opts.hover
            && self.frame_no > 4
            && self.frame_no < 8
            && let Some(r) = ctx.data(|d| d.get_temp::<Rect>(rect_id(id)))
        {
            raw.events.push(egui::Event::PointerMoved(r.center()));
        }
    }

    fn ui(&mut self, ui: &mut Ui, frame: &mut eframe::Frame) {
        self.last_cpu = frame.info().cpu_usage;
        self.draw(ui);
        self.automation(&ui.ctx().clone());
    }
}

/// Run a few frames headless with AccessKit on and summarise the tree it
/// produces: node count, roles, and the labels of buttons.
pub fn a11y_dump(opts: Opts) {
    let ctx = egui::Context::default();
    let fonts = theme::install_fonts(&ctx);
    theme::install_styles(&ctx);
    ctx.set_theme(opts.theme.unwrap_or(ThemePreference::Dark));
    ctx.enable_accesskit();
    let view = opts.view;
    let mut app = App::new(opts);
    let raw = || egui::RawInput {
        screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(1280.0, 800.0))),
        ..Default::default()
    };
    let mut last = None;
    for _ in 0..3 {
        let out = ctx.run_ui(raw(), |ui| app.draw(ui));
        if let Some(u) = out.platform_output.accesskit_update {
            last = Some(u);
        }
    }
    let Some(update) = last else {
        println!("a11y view={view:?} no accesskit update produced");
        return;
    };
    let mut roles: std::collections::BTreeMap<String, usize> = Default::default();
    let mut buttons = Vec::new();
    let mut labelled = 0;
    for (_, node) in &update.nodes {
        *roles.entry(format!("{:?}", node.role())).or_default() += 1;
        if node.label().is_some() || node.value().is_some() {
            labelled += 1;
        }
        if node.role() == egui::accesskit::Role::Button {
            let name = node.label().unwrap_or("?").to_string();
            let off = if node.is_disabled() { " (disabled)" } else { "" };
            buttons.push(format!("{name}{off}"));
        }
    }
    println!("a11y view={view:?} fonts={fonts} nodes={} labelled={labelled} focus={:?}", update.nodes.len(), update.focus);
    println!("a11y roles {roles:?}");
    println!("a11y buttons {}", buttons.join(", "));
    let sample: Vec<String> = update
        .nodes
        .iter()
        .filter(|(_, n)| n.role() == egui::accesskit::Role::Label || n.role() == egui::accesskit::Role::TreeItem || n.role() == egui::accesskit::Role::ListItem)
        .take(12)
        .map(|(_, n)| format!("{:?}:{}", n.role(), n.label().or(n.value()).unwrap_or("")))
        .collect();
    println!("a11y sample {}", sample.join(" ; "));
}
