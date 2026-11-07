//! A flame graph (icicle layout, root on top) for a generated profile.
//! Frames are stored in preorder with the index one past their subtree,
//! so painting skips a whole subtree when it is off screen or narrower
//! than a pixel. Hit testing uses a per-depth list sorted by start.

use eframe::egui::{self, Align2, Color32, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, pos2, vec2};

use crate::theme::{self, size};
use crate::tree::mix;

pub struct Frame {
    pub start: f64,
    pub dur: f64,
    pub self_time: f64,
    pub name: u32,
    pub depth: u16,
    pub end: u32,
}

pub struct Profile {
    pub frames: Vec<Frame>,
    pub names: Vec<String>,
    by_depth: Vec<Vec<u32>>,
    pub max_depth: u16,
    pub total: f64,
    /// Visible time window.
    view: (f64, f64),
    y_offset: f32,
    pub hovered: Option<u32>,
    pub selected: Option<u32>,
    pub drawn_last_frame: usize,
}

const NS: [&str; 8] = ["App\\Http\\", "App\\Service\\", "App\\Repository\\", "Vendor\\Orm\\", "Vendor\\Http\\", "Vendor\\Template\\", "App\\Domain\\", "Vendor\\Cache\\"];
const CLS: [&str; 10] = ["OrderController", "OrderService", "PaymentGateway", "EntityManager", "QueryBuilder", "Router", "Renderer", "Serializer", "CachePool", "EventDispatcher"];
const FN: [&str; 12] = ["handle", "charge", "find", "flush", "execute", "dispatch", "render", "normalize", "get", "build", "resolve", "validate"];

struct Gen {
    frames: Vec<Frame>,
    seed: u64,
}

impl Gen {
    fn next(&mut self) -> u64 {
        self.seed = mix(self.seed);
        self.seed
    }

    /// Emit a frame and `budget - 1` descendants within [start, start+dur).
    fn emit(&mut self, start: f64, dur: f64, depth: u16, budget: usize, names: usize) {
        let me = self.frames.len();
        let name = (self.next() % names as u64) as u32;
        self.frames.push(Frame { start, dur, self_time: dur, name, depth, end: 0 });
        let mut rest = budget - 1;
        if rest > 0 && depth < 120 {
            let r = self.next();
            // Fan-out: often 1 (deep stacks), sometimes wide.
            let k = match r % 10 {
                0..=3 => 1,
                4..=6 => 2 + (r >> 8) % 3,
                _ => 3 + (r >> 8) % 8,
            }
            .min(rest as u64) as usize;
            let mut weights: Vec<f64> = (0..k).map(|_| 0.2 + (self.next() % 1000) as f64 / 1000.0).collect();
            let wsum: f64 = weights.iter().sum();
            weights.iter_mut().for_each(|w| *w /= wsum);
            let self_frac = 0.02 + (self.next() % 12) as f64 / 100.0;
            let child_time = dur * (1.0 - self_frac);
            let mut t = start + dur * self_frac * 0.5;
            let mut used_time = 0.0;
            for (c, w) in weights.iter().enumerate() {
                let b = if c + 1 == k { rest } else { ((rest as f64 * w) as usize).clamp(1, rest - (k - c - 1)) };
                let d = child_time * w;
                self.emit(t, d, depth + 1, b, names);
                t += d;
                used_time += d;
                rest -= b;
            }
            self.frames[me].self_time = dur - used_time;
        }
        let end = self.frames.len() as u32;
        self.frames[me].end = end;
    }
}

impl Profile {
    pub fn generated(n: usize) -> Self {
        let mut names = Vec::new();
        for (a, ns) in NS.iter().enumerate() {
            for (b, c) in CLS.iter().enumerate() {
                for f in FN {
                    if (a + b) % 2 == 0 || f.len() % 2 == 0 {
                        names.push(format!("{ns}{c}::{f}"));
                    }
                }
            }
        }
        names.insert(0, "{main}".into());
        let total = 1_250_000.0; // microseconds
        let mut g = Gen { frames: Vec::with_capacity(n), seed: 42 };
        g.emit(0.0, total, 0, n, names.len());
        g.frames[0].name = 0;
        let max_depth = g.frames.iter().map(|f| f.depth).max().unwrap_or(0);
        let mut by_depth = vec![Vec::new(); max_depth as usize + 1];
        for (i, f) in g.frames.iter().enumerate() {
            by_depth[f.depth as usize].push(i as u32);
        }
        for v in &mut by_depth {
            v.sort_by(|a, b| g.frames[*a as usize].start.total_cmp(&g.frames[*b as usize].start));
        }
        Profile { frames: g.frames, names, by_depth, max_depth, total, view: (0.0, total), y_offset: 0.0, hovered: None, selected: None, drawn_last_frame: 0 }
    }

    fn hit(&self, t: f64, depth: usize) -> Option<u32> {
        let row = self.by_depth.get(depth)?;
        let k = row.partition_point(|&i| self.frames[i as usize].start <= t);
        let i = *row.get(k.checked_sub(1)?)?;
        let f = &self.frames[i as usize];
        (t < f.start + f.dur).then_some(i)
    }

    pub fn zoom(&mut self, factor: f64, at: f64) {
        let (a, b) = self.view;
        let w = ((b - a) / factor).clamp(1.0, self.total);
        let frac = ((at - a) / (b - a)).clamp(0.0, 1.0);
        let na = (at - w * frac).clamp(0.0, self.total - w);
        self.view = (na, na + w);
    }

    pub fn zoom_to(&mut self, i: u32) {
        let f = &self.frames[i as usize];
        self.view = (f.start, f.start + f.dur.max(1.0));
    }

    pub fn reset(&mut self) {
        self.view = (0.0, self.total);
    }

    pub fn view_fraction(&self) -> f64 {
        (self.view.1 - self.view.0) / self.total
    }
}

pub const ROW_H: f32 = 20.0;

fn color_for(p: &theme::Palette, name: &str, h: u64) -> Color32 {
    // Warm hues for app code, cooler for vendor code.
    let vendor = name.starts_with("Vendor");
    let t = (h % 1000) as f32 / 1000.0;
    let (r, g, b) = if vendor {
        (0.36 + 0.10 * t, 0.52 + 0.12 * t, 0.78 + 0.06 * t)
    } else {
        (0.92 + 0.06 * t, 0.42 + 0.30 * t, 0.22 + 0.10 * t)
    };
    let k = if p.dark { 0.78 } else { 1.0 };
    Color32::from_rgb((r * k * 255.0) as u8, (g * k * 255.0) as u8, (b * k * 255.0) as u8)
}

fn ms(us: f64) -> String {
    if us >= 1000.0 { format!("{:.1} ms", us / 1000.0) } else { format!("{us:.0} µs") }
}

/// Paint the graph into the remaining space.
pub fn show(ui: &mut Ui, prof: &mut Profile) {
    let p = theme::palette(ui.ctx());
    let (rect, resp) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, size::RADIUS, p.surface);

    // Input: wheel scrolls vertically, Ctrl+wheel (or pinch) zooms around
    // the pointer, drag pans, double click zooms to a frame.
    let (v0, v1) = prof.view;
    let scale = rect.width() as f64 / (v1 - v0);
    let to_t = |x: f32| v0 + (x - rect.left()) as f64 / scale;
    if let Some(pos) = resp.hover_pos() {
        let (zoom, scroll) = ui.input(|i| (i.zoom_delta(), i.smooth_scroll_delta));
        if zoom != 1.0 {
            prof.zoom(zoom as f64, to_t(pos.x));
        }
        if scroll.y != 0.0 {
            let content_h = (prof.max_depth as f32 + 1.0) * ROW_H;
            prof.y_offset = (prof.y_offset - scroll.y).clamp(0.0, (content_h - rect.height()).max(0.0));
        }
        if scroll.x != 0.0 {
            let dt = -scroll.x as f64 / scale;
            let w = v1 - v0;
            let na = (v0 + dt).clamp(0.0, prof.total - w);
            prof.view = (na, na + w);
        }
    }
    if resp.dragged() {
        let d = resp.drag_delta();
        let w = prof.view.1 - prof.view.0;
        let na = (prof.view.0 - d.x as f64 / scale).clamp(0.0, prof.total - w);
        prof.view = (na, na + w);
        let content_h = (prof.max_depth as f32 + 1.0) * ROW_H;
        prof.y_offset = (prof.y_offset - d.y).clamp(0.0, (content_h - rect.height()).max(0.0));
    }

    let (v0, v1) = prof.view;
    let scale = rect.width() as f64 / (v1 - v0);
    let top = rect.top() - prof.y_offset;
    let pointer = resp.hover_pos();
    prof.hovered = pointer.and_then(|pos: Pos2| {
        let depth = ((pos.y - top) / ROW_H).floor();
        if depth < 0.0 { None } else { prof.hit(v0 + (pos.x - rect.left()) as f64 / scale, depth as usize) }
    });
    if resp.double_clicked() {
        if let Some(h) = prof.hovered {
            prof.zoom_to(h);
        }
    } else if resp.clicked() {
        prof.selected = prof.hovered;
    }

    let font = theme::mono(11.5);
    let char_w = ui.fonts_mut(|f| f.glyph_width(&font, 'm'));
    let label_color = Color32::from_rgb(0x1A, 0x1A, 0x1A);
    let mut i = 0usize;
    let mut drawn = 0usize;
    let n = prof.frames.len();
    while i < n {
        let f = &prof.frames[i];
        let x0 = rect.left() as f64 + (f.start - v0) * scale;
        let x1 = x0 + f.dur * scale;
        let y = top + f.depth as f32 * ROW_H;
        if x1 < rect.left() as f64 || x0 > rect.right() as f64 || (x1 - x0) < 0.6 || y > rect.bottom() {
            i = f.end as usize;
            continue;
        }
        if y + ROW_H >= rect.top() {
            let xa = x0.max(rect.left() as f64 - 2.0) as f32;
            let xb = x1.min(rect.right() as f64 + 2.0) as f32;
            let r = Rect::from_min_max(pos2(xa, y), pos2(xb - 1.0, y + ROW_H - 1.0));
            let name = &prof.names[f.name as usize];
            let mut c = color_for(p, name, mix(f.name as u64));
            if prof.hovered == Some(i as u32) {
                c = c.gamma_multiply(1.15);
            }
            painter.rect_filled(r, 2, c);
            let w = r.width() - 8.0;
            if w > char_w * 3.0 {
                let fit = (w / char_w) as usize;
                let text = if name.chars().count() <= fit {
                    name.clone()
                } else {
                    let short: String = name.rsplit('\\').next().unwrap_or(name).chars().take(fit.saturating_sub(1)).collect();
                    format!("{short}…")
                };
                painter.text(pos2(r.left() + 4.0, r.center().y), Align2::LEFT_CENTER, text, font.clone(), label_color);
            }
            drawn += 1;
        }
        i += 1;
    }
    prof.drawn_last_frame = drawn;
    for (id, col) in [(prof.selected, p.text), (prof.hovered, p.accent)] {
        if let Some(h) = id {
            let f = &prof.frames[h as usize];
            let x0 = rect.left() + ((f.start - v0) * scale) as f32;
            let r = Rect::from_min_size(pos2(x0, top + f.depth as f32 * ROW_H), vec2((f.dur * scale) as f32, ROW_H - 1.0));
            painter.rect_stroke(r, 2, Stroke::new(1.5, col), StrokeKind::Outside);
        }
    }
    if let Some(h) = prof.hovered {
        let f = &prof.frames[h as usize];
        let name = prof.names[f.name as usize].clone();
        let (total, self_t, depth, pct) = (f.dur, f.self_time, f.depth, f.dur / prof.total * 100.0);
        resp.on_hover_ui_at_pointer(|ui| {
            ui.label(egui::RichText::new(name).font(theme::mono(size::MONO)));
            ui.label(format!("total {}  ({pct:.2}%)", ms(total)));
            ui.label(format!("self {}   depth {depth}", ms(self_t)));
            ui.label(egui::RichText::new("Double click to zoom").color(p.text_weak));
        });
    }
}

/// The line under the graph that names the frame under the cursor.
pub fn status(prof: &Profile) -> String {
    match prof.hovered.or(prof.selected) {
        Some(h) => {
            let f = &prof.frames[h as usize];
            format!(
                "{}   total {}   self {}   {:.2}% of request   depth {}",
                prof.names[f.name as usize],
                ms(f.dur),
                ms(f.self_time),
                f.dur / prof.total * 100.0,
                f.depth
            )
        }
        None => format!("{} frames, depth {}, {} total. Hover a frame.", prof.frames.len(), prof.max_depth + 1, ms(prof.total)),
    }
}
