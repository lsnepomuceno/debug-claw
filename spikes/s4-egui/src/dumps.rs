//! The dump feed: cards of mixed height (text, SQL, tables, collapsed
//! object trees). Card heights come from the content, so a prefix sum
//! gives every card's offset without laying it out, and only the cards
//! inside the viewport are built each frame.

use std::collections::HashMap;

use eframe::egui::{
    self, Align2, Color32, CornerRadius, Label, Rect, RichText, ScrollArea, Sense, Stroke,
    StrokeKind, Ui, UiBuilder, pos2, vec2,
};
use egui_phosphor::regular as ph;

use crate::actions::{self, ActionId};
use crate::theme::{self, DumpColor, size};
use crate::tree::mix;

pub enum Body {
    Text(Vec<String>),
    Query { sql: String, bindings: String, ms: f32 },
    Table { cols: Vec<String>, rows: Vec<Vec<String>> },
    Tree(DumpTree),
}

/// A small object graph, in preorder. `end` is one past the last
/// descendant, so a collapsed node skips its subtree in one step.
pub struct DumpTree {
    pub nodes: Vec<DNode>,
}

pub struct DNode {
    pub depth: u8,
    pub key: String,
    pub value: String,
    pub end: u32,
}

pub struct Dump {
    pub color: DumpColor,
    pub label: &'static str,
    pub request: &'static str,
    pub origin: String,
    pub time: String,
    pub body: Body,
    pub screen: &'static str,
}

pub struct Feed {
    pub dumps: Vec<Dump>,
    /// Expanded tree nodes, per dump index.
    open: HashMap<u32, Vec<u32>>,
    /// Indices shown after filtering, and their offsets.
    visible: Vec<u32>,
    offsets: Vec<f32>,
    pub filter_color: Option<DumpColor>,
    pub filter_label: Option<&'static str>,
    pub filter_screen: Option<&'static str>,
    scroll_to: Option<f32>,
}

const HEAD: f32 = 34.0;
const LINE: f32 = 19.0;
const PAD_BOTTOM: f32 = 12.0;
const GAP: f32 = 8.0;
const MAX_LINES: usize = 10;

const LABELS: [&str; 6] = ["orders", "query", "cart", "auth", "payload", "debug"];
const REQUESTS: [&str; 6] = ["GET /orders/42", "POST /orders", "GET /cart", "POST /login", "GET /api/products?page=3", "PUT /orders/42/items"];
const FILES: [&str; 6] = ["OrderService.php", "Repository.php", "CartController.php", "AuthMiddleware.php", "ProductApi.php", "OrderItemService.php"];
const SCREENS: [&str; 3] = ["default", "orders", "checkout"];

fn order_tree(id: u64, items: usize) -> DumpTree {
    let mut nodes = Vec::new();
    let mut push = |depth: u8, key: &str, value: String| {
        nodes.push(DNode { depth, key: key.into(), value, end: 0 });
    };
    let h = mix(id);
    let total = if id % 3 == 0 { "0.0".to_string() } else { format!("{}.{:02}", 10 + h % 400, h % 100) };
    push(0, "", format!("App\\Order {{ id: {id}, total: {total}, items: [{items}] }}"));
    push(1, "id", id.to_string());
    push(1, "total", total.clone());
    push(1, "status", format!("\"{}\"", ["paid", "pending", "failed"][(h % 3) as usize]));
    push(1, "customer", format!("App\\Customer {{ id: {}, … }}", 5000 + id % 900));
    push(2, "id", (5000 + id % 900).to_string());
    push(2, "email", format!("\"customer{}@example.com\"", id % 900));
    push(1, "items", format!("array({items})"));
    for j in 0..items {
        push(2, &format!("[{j}]"), format!("App\\OrderItem {{ sku: \"SKU-{:04}\", … }}", (h >> j) % 10_000));
        push(3, "sku", format!("\"SKU-{:04}\"", (h >> j) % 10_000));
        push(3, "qty", (1 + (h >> (j + 3)) % 4).to_string());
    }
    // Fill in `end` from the depths.
    let n = nodes.len();
    for i in 0..n {
        let d = nodes[i].depth;
        let end = (i + 1..n).find(|&k| nodes[k].depth <= d).unwrap_or(n);
        nodes[i].end = end as u32;
    }
    DumpTree { nodes }
}

fn generated(i: usize) -> Dump {
    let h = mix(i as u64 * 7919);
    let color = DumpColor::ALL[(h % 7) as usize];
    let label = LABELS[((h >> 3) % 6) as usize];
    let request = REQUESTS[((h >> 7) % 6) as usize];
    let origin = format!("{}:{}", FILES[((h >> 11) % 6) as usize], 10 + (h >> 13) % 300);
    let secs = 3600 * 12 + i as u64 * 2;
    let time = format!("{:02}:{:02}:{:02}", secs / 3600 % 24, secs / 60 % 60, secs % 60);
    let kind = (h >> 20) % 100;
    let body = if kind < 35 {
        let lines = 1 + ((h >> 30) % 4) as usize;
        Body::Text(
            (0..lines)
                .map(|l| match l {
                    0 => format!("\"Charging order {} for customer {}\"", 1000 + i, 5000 + h % 900),
                    1 => format!("gateway: \"stripe\", attempt: {}, idempotency_key: \"{:016x}\"", 1 + h % 3, h),
                    2 => "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor".into(),
                    _ => format!("memory: {} KB, peak: {} KB", 2048 + h % 4096, 4096 + h % 8192),
                })
                .collect(),
        )
    } else if kind < 55 {
        Body::Query {
            sql: ["SELECT * FROM orders WHERE id = ?", "SELECT o.*, c.email FROM orders o JOIN customers c ON c.id = o.customer_id WHERE o.status = ? LIMIT 50", "UPDATE orders SET total = ?, updated_at = ? WHERE id = ?", "INSERT INTO order_items (order_id, sku, qty) VALUES (?, ?, ?)"][((h >> 30) % 4) as usize].into(),
            bindings: format!("[{}, \"{}\"]", 1000 + i, ["paid", "pending"][(h % 2) as usize]),
            ms: 0.2 + (h % 400) as f32 / 100.0,
        }
    } else if kind < 70 {
        let rows = 2 + ((h >> 30) % 5) as usize;
        Body::Table {
            cols: vec!["sku".into(), "qty".into(), "price".into(), "subtotal".into()],
            rows: (0..rows)
                .map(|r| {
                    let x = mix(h ^ r as u64);
                    let qty = 1 + x % 4;
                    let price = 1 + (x >> 8) % 200;
                    vec![format!("SKU-{:04}", x % 10_000), qty.to_string(), format!("{price}.00"), format!("{}.00", qty * price)]
                })
                .collect(),
        }
    } else {
        Body::Tree(order_tree(1000 + i as u64, 1 + ((h >> 30) % 4) as usize))
    };
    Dump { color, label, request, origin, time, body, screen: SCREENS[((h >> 40) % 3) as usize] }
}

/// The first cards match the sketch in docs/ui.md, so the feed mockup
/// starts with them.
fn curated() -> Vec<Dump> {
    vec![
        Dump { color: DumpColor::Red, label: "orders", request: "GET /orders/42", origin: "OrderService.php:42".into(), time: "12:01:07".into(), body: Body::Tree(order_tree(42, 3)), screen: "orders" },
        Dump {
            color: DumpColor::Blue,
            label: "query",
            request: "GET /orders/42",
            origin: "Repository.php:88".into(),
            time: "12:01:07".into(),
            body: Body::Query { sql: "SELECT * FROM orders WHERE id = ?".into(), bindings: "[42]".into(), ms: 1.2 },
            screen: "orders",
        },
        Dump {
            color: DumpColor::Green,
            label: "cart",
            request: "GET /cart",
            origin: "CartController.php:31".into(),
            time: "12:00:58".into(),
            body: Body::Table {
                cols: vec!["sku".into(), "qty".into(), "price".into(), "subtotal".into()],
                rows: vec![
                    vec!["SKU-1042".into(), "2".into(), "19.90".into(), "39.80".into()],
                    vec!["SKU-2210".into(), "1".into(), "149.00".into(), "149.00".into()],
                    vec!["SKU-0007".into(), "3".into(), "4.50".into(), "13.50".into()],
                ],
            },
            screen: "checkout",
        },
        Dump {
            color: DumpColor::Orange,
            label: "payload",
            request: "POST /orders",
            origin: "OrderController.php:27".into(),
            time: "12:00:55".into(),
            body: Body::Text(vec!["\"customer_id\" => \"3\"".into(), "\"items\" => \"[{\\\"sku\\\":\\\"SKU-1042\\\",\\\"qty\\\":2}]\"".into()]),
            screen: "default",
        },
        Dump { color: DumpColor::Purple, label: "auth", request: "POST /login", origin: "AuthMiddleware.php:64".into(), time: "12:00:41".into(), body: Body::Text(vec!["\"User 812 signed in with remember_me = true\"".into()]), screen: "default" },
    ]
}

impl Body {
    fn lines(&self, open: Option<&Vec<u32>>) -> usize {
        match self {
            Body::Text(l) => l.len().min(MAX_LINES),
            Body::Query { .. } => 2,
            Body::Table { rows, .. } => rows.len().min(MAX_LINES) + 1,
            Body::Tree(t) => visible_tree_rows(t, open).len(),
        }
    }
}

fn visible_tree_rows(t: &DumpTree, open: Option<&Vec<u32>>) -> Vec<u32> {
    let mut out = Vec::new();
    let mut i = 0u32;
    while (i as usize) < t.nodes.len() {
        out.push(i);
        let n = &t.nodes[i as usize];
        let has_kids = n.end > i + 1;
        let is_open = open.is_some_and(|o| o.contains(&i));
        i = if has_kids && !is_open { n.end } else { i + 1 };
    }
    out
}

impl Feed {
    pub fn new(count: usize, with_curated: bool) -> Self {
        let mut dumps = if with_curated { curated() } else { Vec::new() };
        let start = dumps.len();
        dumps.extend((start..count).map(generated));
        let mut f = Feed {
            dumps,
            open: HashMap::new(),
            visible: Vec::new(),
            offsets: Vec::new(),
            filter_color: None,
            filter_label: None,
            filter_screen: None,
            scroll_to: None,
        };
        f.refilter();
        f
    }

    fn height(&self, i: u32) -> f32 {
        let d = &self.dumps[i as usize];
        HEAD + d.body.lines(self.open.get(&i)) as f32 * LINE + PAD_BOTTOM
    }

    pub fn refilter(&mut self) {
        self.visible = (0..self.dumps.len() as u32)
            .filter(|&i| {
                let d = &self.dumps[i as usize];
                self.filter_color.is_none_or(|c| c == d.color)
                    && self.filter_label.is_none_or(|l| l == d.label)
                    && self.filter_screen.is_none_or(|s| s == d.screen)
            })
            .collect();
        self.recompute_offsets();
    }

    fn recompute_offsets(&mut self) {
        self.offsets.clear();
        let mut y = 0.0;
        for &i in &self.visible {
            self.offsets.push(y);
            y += self.height(i) + GAP;
        }
        self.offsets.push(y);
    }

    pub fn total_height(&self) -> f32 {
        *self.offsets.last().unwrap_or(&0.0)
    }

    pub fn shown(&self) -> usize {
        self.visible.len()
    }

    pub fn counts<K: Eq + std::hash::Hash + Copy>(&self, key: impl Fn(&Dump) -> K) -> HashMap<K, usize> {
        let mut m = HashMap::new();
        for d in &self.dumps {
            *m.entry(key(d)).or_insert(0) += 1;
        }
        m
    }

    fn toggle(&mut self, dump: u32, node: u32) {
        let v = self.open.entry(dump).or_default();
        if let Some(p) = v.iter().position(|&n| n == node) {
            v.remove(p);
        } else {
            v.push(node);
        }
        self.recompute_offsets();
    }

    pub fn first_visible_index(&self, y: f32) -> usize {
        self.offsets.partition_point(|&o| o <= y).saturating_sub(1)
    }
}

/// The virtualised feed. `hover_actions` shows copy, pin, open and hide
/// on the hovered card.
pub fn show(ui: &mut Ui, feed: &mut Feed, id_salt: &str) {
    let p = theme::palette(ui.ctx());
    let mut toggle: Option<(u32, u32)> = None;
    let mut area = ScrollArea::vertical().id_salt(id_salt).auto_shrink(false);
    if let Some(y) = feed.scroll_to.take() {
        area = area.vertical_scroll_offset(y);
    }
    area.show_viewport(ui, |ui, viewport| {
        let width = ui.available_width();
        ui.set_height(feed.total_height());
        let origin = ui.max_rect().min;
        let first = feed.first_visible_index(viewport.min.y);
        let mut k = first;
        while k < feed.visible.len() && feed.offsets[k] < viewport.max.y {
            let i = feed.visible[k];
            let h = feed.offsets[k + 1] - feed.offsets[k] - GAP;
            let rect = Rect::from_min_size(origin + vec2(0.0, feed.offsets[k]), vec2(width, h));
            let open = feed.open.get(&i);
            if let Some(t) = card(ui, p, rect, &feed.dumps[i as usize], i, open) {
                toggle = Some((i, t));
            }
            k += 1;
        }
    });
    if let Some((d, n)) = toggle {
        feed.toggle(d, n);
    }
}

fn card(ui: &mut Ui, p: &theme::Palette, rect: Rect, d: &Dump, idx: u32, open: Option<&Vec<u32>>) -> Option<u32> {
    let mut toggled = None;
    let resp = ui.interact(rect, ui.scope_id().with(("card", idx)), Sense::hover());
    let hovered = resp.hovered() || ui.rect_contains_pointer(rect);
    let painter = ui.painter().clone();
    let r = CornerRadius::same(size::RADIUS);
    painter.rect_filled(rect, r, p.surface);
    painter.rect_stroke(rect, r, Stroke::new(1.0, if hovered { p.text_weak.gamma_multiply(0.5) } else { p.border }), StrokeKind::Inside);
    let edge = Rect::from_min_size(rect.min, vec2(4.0, rect.height()));
    painter.rect_filled(edge, CornerRadius { nw: size::RADIUS, sw: size::RADIUS, ne: 0, se: 0 }, d.color.color());

    // Header: label pill, request, origin link, time or hover actions.
    let hy = rect.top() + 17.0;
    let x0 = rect.left() + 16.0;
    let label_font = theme::medium(size::SMALL);
    let galley = ui.fonts_mut(|f| f.layout_no_wrap(d.label.to_owned(), label_font, d.color.color()));
    let pill = Rect::from_min_size(pos2(x0, hy - 10.0), vec2(galley.size().x + 14.0, 20.0));
    painter.rect_filled(pill, 10, d.color.color().gamma_multiply(if p.dark { 0.18 } else { 0.12 }));
    painter.galley(pos2(pill.left() + 7.0, hy - galley.size().y / 2.0), galley, d.color.color());
    let mut x = pill.right() + 12.0;
    let req = painter.text(pos2(x, hy), Align2::LEFT_CENTER, d.request, theme::body(size::SMALL), p.text_weak);
    x = req.right() + 14.0;
    let link_rect = Rect::from_min_size(pos2(x, hy - 9.0), vec2(170.0, 18.0));
    let link = put_left(ui, link_rect, egui::Link::new(theme::icon_job(ph::FILE_CODE, &d.origin, theme::body(size::SMALL), p.accent)));
    link.on_hover_text("Open in editor");

    let right = rect.right() - 12.0;
    if hovered {
        let bar = Rect::from_min_max(pos2(right - 118.0, hy - 12.0), pos2(right, hy + 12.0));
        ui.scope_builder(UiBuilder::new().max_rect(bar).layout(egui::Layout::right_to_left(egui::Align::Center)), |ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            actions::button(ui, ActionId::HideSimilar, actions::Style::Ghost, None);
            actions::button(ui, ActionId::OpenInEditor, actions::Style::Ghost, Some("Needs an editor in Settings"));
            actions::button(ui, ActionId::PinDump, actions::Style::Ghost, None);
            actions::button(ui, ActionId::CopyDump, actions::Style::Ghost, None);
        });
    } else {
        painter.text(pos2(right, hy), Align2::RIGHT_CENTER, &d.time, theme::mono(size::SMALL), p.text_weak);
    }

    // Body.
    let bx = x0;
    let mut y = rect.top() + HEAD;
    let mono = theme::mono(size::MONO);
    let line_rect = |y: f32, indent: f32| Rect::from_min_size(pos2(bx + indent, y), vec2(rect.right() - bx - indent - 12.0, LINE));
    match &d.body {
        Body::Text(lines) => {
            for l in lines.iter().take(MAX_LINES) {
                put_left(ui, line_rect(y, 0.0), Label::new(RichText::new(l).font(mono.clone()).color(p.value.string)).selectable(true).truncate());
                y += LINE;
            }
        }
        Body::Query { sql, bindings, ms } => {
            put_left(ui, line_rect(y, 0.0), Label::new(RichText::new(sql).font(mono.clone()).color(p.syntax.keyword)).selectable(true).truncate());
            y += LINE;
            let job = RichText::new(format!("{bindings}    {ms:.1} ms")).font(mono.clone()).color(p.text_weak);
            put_left(ui, line_rect(y, 0.0), Label::new(job).selectable(true));
        }
        Body::Table { cols, rows } => {
            let cw = 120.0;
            for (c, name) in cols.iter().enumerate() {
                painter.text(pos2(bx + c as f32 * cw, y + LINE / 2.0), Align2::LEFT_CENTER, name, theme::medium(size::SMALL), p.text_weak);
            }
            painter.hline(bx..=bx + cw * cols.len() as f32, y + LINE - 1.0, Stroke::new(1.0, p.border));
            y += LINE;
            for row in rows.iter().take(MAX_LINES) {
                for (c, v) in row.iter().enumerate() {
                    let r = Rect::from_min_size(pos2(bx + c as f32 * cw, y), vec2(cw - 8.0, LINE));
                    put_left(ui, r, Label::new(RichText::new(v).font(mono.clone())).selectable(true));
                }
                y += LINE;
            }
        }
        Body::Tree(t) => {
            for ni in visible_tree_rows(t, open) {
                let n = &t.nodes[ni as usize];
                let indent = n.depth as f32 * 16.0;
                let has_kids = n.end > ni + 1;
                let is_open = open.is_some_and(|o| o.contains(&ni));
                if has_kids {
                    let caret = Rect::from_min_size(pos2(bx + indent - 2.0, y), vec2(16.0, LINE));
                    let cr = ui.interact(caret, ui.scope_id().with(("caret", idx, ni)), Sense::click());
                    let col = if cr.hovered() { p.text } else { p.text_weak };
                    painter.text(caret.center(), Align2::CENTER_CENTER, if is_open { ph::CARET_DOWN } else { ph::CARET_RIGHT }, theme::icon_font(12.0), col);
                    if cr.clicked() {
                        toggled = Some(ni);
                    }
                }
                let text = if n.key.is_empty() { n.value.clone() } else { format!("{}: {}", n.key, n.value) };
                let color = if n.depth == 0 { p.text } else { value_color(p, &n.value) };
                put_left(ui, line_rect(y, indent + 16.0), Label::new(RichText::new(text).font(mono.clone()).color(color)).selectable(true).truncate());
                y += LINE;
            }
        }
    }
    toggled
}

/// Like `Ui::put`, but left-aligned and vertically centred in `rect`
/// (`put` centres the widget).
pub fn put_left(ui: &mut Ui, rect: Rect, w: impl egui::Widget) -> egui::Response {
    ui.scope_builder(UiBuilder::new().max_rect(rect).layout(egui::Layout::left_to_right(egui::Align::Center)), |ui| ui.add(w)).inner
}

fn value_color(p: &theme::Palette, v: &str) -> Color32 {
    if v.starts_with('"') {
        p.value.string
    } else if v.as_bytes().first().is_some_and(u8::is_ascii_digit) {
        p.value.number
    } else {
        p.text
    }
}
