//! A variable tree that fetches children only when a node is expanded,
//! the way the app will read Xdebug one level at a time. The generated
//! tree stands in for the daemon: `$orders` holds `n` orders of 50 nodes
//! each, and a node's children are built from its key on first expand.
//!
//! The view keeps a flat list of visible rows and paints only the rows
//! inside the scroll viewport (`ScrollArea::show_rows`).

use eframe::egui::{
    self, Align2, Color32, ScrollArea, Sense, Ui, WidgetInfo, text::LayoutJob, vec2,
};
use egui_phosphor::regular as ph;

use crate::theme::{self, size};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Str,
    Num,
    Bool,
    #[allow(dead_code)]
    Null,
    Array,
    Object,
}

#[derive(Clone, Copy)]
enum Key {
    Root,
    Order(u32),
    Customer(u32),
    Address(u32),
    Items(u32),
    Item(u32, u8),
    /// Pre-built children (hand-made trees) or no children at all.
    Fixed,
}

pub struct Node {
    pub name: Box<str>,
    pub value: Box<str>,
    pub ty: Box<str>,
    pub kind: Kind,
    pub child_count: u32,
    pub changed: bool,
    key: Key,
    children: Option<(u32, u32)>,
    expanded: bool,
}

#[derive(Clone, Copy)]
pub struct Row {
    pub node: u32,
    pub depth: u16,
}

pub struct VarTree {
    nodes: Vec<Node>,
    roots: Vec<u32>,
    pub rows: Vec<Row>,
    pub selected: Option<u32>,
    /// Size of the whole tree, fetched or not.
    pub virtual_nodes: usize,
    /// A row to scroll to on the next frame.
    scroll_to_row: Option<usize>,
}

fn node(name: impl Into<Box<str>>, kind: Kind, value: impl Into<Box<str>>, ty: &str) -> Node {
    Node {
        name: name.into(),
        value: value.into(),
        ty: ty.into(),
        kind,
        child_count: 0,
        changed: false,
        key: Key::Fixed,
        children: None,
        expanded: false,
    }
}

fn compound(name: impl Into<Box<str>>, kind: Kind, value: impl Into<Box<str>>, ty: &str, n: u32, key: Key) -> Node {
    Node { child_count: n, key, ..node(name, kind, value, ty) }
}

/// A small deterministic hash, so values look varied without a RNG.
pub fn mix(a: u64) -> u64 {
    let mut x = a.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

const STATUSES: [&str; 4] = ["paid", "pending", "failed", "refunded"];
const NAMES: [&str; 8] = ["Ana Souza", "Bruno Lima", "Carla Dias", "Diego Alves", "Elisa Rocha", "Fábio Nunes", "Gabi Melo", "Hugo Reis"];
const CITIES: [&str; 6] = ["São Paulo", "Recife", "Curitiba", "Porto Alegre", "Belo Horizonte", "Salvador"];

fn generate(key: Key) -> Vec<Node> {
    match key {
        Key::Root | Key::Fixed => Vec::new(),
        Key::Order(i) => {
            let h = mix(i as u64);
            let total = if i % 3 == 2 { "0.0".to_string() } else { format!("{}.{:02}", 10 + h % 990, h % 100) };
            vec![
                node("id", Kind::Num, (1000 + i).to_string(), "int"),
                node("status", Kind::Str, format!("\"{}\"", STATUSES[(h % 4) as usize]), "string"),
                node("total", Kind::Num, total, "float"),
                node("createdAt", Kind::Str, format!("\"2026-11-07 {:02}:{:02}:{:02}\"", h % 24, (h >> 8) % 60, (h >> 16) % 60), "string"),
                compound("customer", Kind::Object, format!("#{}", 5000 + i), "App\\Customer", 4, Key::Customer(i)),
                compound("items", Kind::Array, "array(9)", "array", 9, Key::Items(i)),
            ]
        }
        Key::Customer(i) => {
            let h = mix(i as u64 ^ 0xC0FFEE);
            let name = NAMES[(h % 8) as usize];
            vec![
                node("id", Kind::Num, (5000 + i).to_string(), "int"),
                node("name", Kind::Str, format!("\"{name}\""), "string"),
                node("email", Kind::Str, format!("\"customer{}@example.com\"", 5000 + i), "string"),
                compound("address", Kind::Object, format!("#{}", 9000 + i), "App\\Address", 3, Key::Address(i)),
            ]
        }
        Key::Address(i) => {
            let h = mix(i as u64 ^ 0xADD);
            vec![
                node("street", Kind::Str, format!("\"Rua {} {}\"", ["das Flores", "XV de Novembro", "Augusta"][(h % 3) as usize], h % 2000), "string"),
                node("city", Kind::Str, format!("\"{}\"", CITIES[(h % 6) as usize]), "string"),
                node("zip", Kind::Str, format!("\"{:05}-{:03}\"", h % 100_000, (h >> 20) % 1000), "string"),
            ]
        }
        Key::Items(i) => (0..9u8)
            .map(|j| compound(format!("[{j}]"), Kind::Object, format!("#{}", 20_000 + i * 9 + j as u32), "App\\OrderItem", 3, Key::Item(i, j)))
            .collect(),
        Key::Item(i, j) => {
            let h = mix(((i as u64) << 8) | j as u64);
            vec![
                node("sku", Kind::Str, format!("\"SKU-{:06}\"", h % 1_000_000), "string"),
                node("qty", Kind::Num, (1 + h % 5).to_string(), "int"),
                node("price", Kind::Num, format!("{}.{:02}", 1 + (h >> 8) % 300, (h >> 16) % 100), "float"),
            ]
        }
    }
}

impl VarTree {
    /// The generated tree: one `$orders` array with `orders` elements,
    /// 50 nodes per order, so `orders = 2000` gives 100 001 nodes.
    pub fn generated(total_nodes: usize) -> Self {
        let orders = (total_nodes.saturating_sub(1) / 50).max(1) as u32;
        let mut root = compound("$orders", Kind::Array, format!("array({orders})"), "array", orders, Key::Root);
        root.changed = false;
        let mut t = VarTree::empty();
        t.nodes.push(root);
        t.roots.push(0);
        t.virtual_nodes = 1 + orders as usize * 50;
        t.rebuild_rows();
        t
    }

    fn empty() -> Self {
        VarTree { nodes: Vec::new(), roots: Vec::new(), rows: Vec::new(), selected: None, virtual_nodes: 0, scroll_to_row: None }
    }

    /// Nodes held in memory (fetched so far).
    pub fn fetched(&self) -> usize {
        self.nodes.len()
    }

    fn ensure_children(&mut self, id: u32) {
        let n = &self.nodes[id as usize];
        if n.children.is_some() || n.child_count == 0 {
            return;
        }
        let kids = match n.key {
            Key::Root => (0..n.child_count)
                .map(|i| compound(format!("[{i}]"), Kind::Object, format!("#{}", 812 + i), "App\\Order", 6, Key::Order(i)))
                .collect(),
            k => generate(k),
        };
        let start = self.nodes.len() as u32;
        let count = kids.len() as u32;
        self.nodes.extend(kids);
        self.nodes[id as usize].children = Some((start, start + count));
    }

    fn push_visible(&self, id: u32, depth: u16, out: &mut Vec<Row>) {
        out.push(Row { node: id, depth });
        let n = &self.nodes[id as usize];
        if n.expanded {
            if let Some((a, b)) = n.children {
                for c in a..b {
                    self.push_visible(c, depth + 1, out);
                }
            }
        }
    }

    fn rebuild_rows(&mut self) {
        let mut rows = Vec::with_capacity(self.rows.len());
        for &r in &self.roots {
            self.push_visible(r, 0, &mut rows);
        }
        self.rows = rows;
    }

    /// Expand or collapse the node shown at `row`, editing only the rows
    /// below it.
    pub fn toggle_row(&mut self, row: usize) {
        let Row { node: id, depth } = self.rows[row];
        if self.nodes[id as usize].child_count == 0 {
            return;
        }
        if self.nodes[id as usize].expanded {
            self.nodes[id as usize].expanded = false;
            let end = self.rows[row + 1..]
                .iter()
                .position(|r| r.depth <= depth)
                .map_or(self.rows.len(), |p| row + 1 + p);
            self.rows.drain(row + 1..end);
        } else {
            self.ensure_children(id);
            self.nodes[id as usize].expanded = true;
            let mut sub = Vec::new();
            self.push_visible(id, depth, &mut sub);
            self.rows.splice(row + 1..row + 1, sub.into_iter().skip(1));
        }
    }

    /// Fetch and expand everything (the worst case for the view).
    pub fn expand_all(&mut self) {
        let mut stack: Vec<u32> = self.roots.clone();
        while let Some(id) = stack.pop() {
            self.ensure_children(id);
            let n = &mut self.nodes[id as usize];
            if n.child_count > 0 {
                n.expanded = true;
            }
            if let Some((a, b)) = n.children {
                stack.extend(a..b);
            }
        }
        self.rebuild_rows();
    }

    pub fn collapse_all(&mut self) {
        for n in &mut self.nodes {
            n.expanded = false;
        }
        self.rebuild_rows();
    }

    /// Expand the first `orders` orders and their customers, the state the
    /// memory comparison uses for both tree sizes.
    pub fn expand_sample(&mut self, orders: usize) {
        if !self.nodes[0].expanded {
            self.toggle_row(0);
        }
        for k in (0..orders.min(self.rows.len() - 1)).rev() {
            self.toggle_row(1 + k);
        }
        self.rebuild_rows();
    }
}

/// A hand-made tree for the debugger mockup.
pub struct Builder {
    tree: VarTree,
}

impl Builder {
    pub fn new() -> Self {
        Builder { tree: VarTree::empty() }
    }

    /// Add a root, or a child of `parent`, and return its id. Children of a
    /// node must be added together, right after it.
    pub fn add(&mut self, parent: Option<u32>, name: &str, kind: Kind, value: &str, ty: &str) -> u32 {
        let id = self.tree.nodes.len() as u32;
        self.tree.nodes.push(node(name, kind, value, ty));
        match parent {
            None => self.tree.roots.push(id),
            Some(p) => {
                let pn = &mut self.tree.nodes[p as usize];
                pn.child_count += 1;
                pn.children = Some(match pn.children {
                    None => (id, id + 1),
                    Some((a, _)) => (a, id + 1),
                });
            }
        }
        id
    }

    pub fn changed(&mut self, id: u32) {
        self.tree.nodes[id as usize].changed = true;
    }

    pub fn expand(&mut self, id: u32) {
        self.tree.nodes[id as usize].expanded = true;
    }

    pub fn build(mut self) -> VarTree {
        self.tree.virtual_nodes = self.tree.nodes.len();
        self.tree.rebuild_rows();
        self.tree
    }
}

fn value_color(p: &theme::Palette, k: Kind) -> Color32 {
    match k {
        Kind::Str => p.value.string,
        Kind::Num => p.value.number,
        Kind::Bool | Kind::Null => p.value.keyword,
        Kind::Array | Kind::Object => p.text,
    }
}

pub const ROW_H: f32 = 22.0;

/// The virtualised tree view. Returns true when the tree changed.
pub fn show(ui: &mut Ui, tree: &mut VarTree, id_salt: &str) -> bool {
    let p = theme::palette(ui.ctx());
    let mut toggled = None;
    let mut changed = false;
    let font = theme::mono(size::MONO);
    let mut area = ScrollArea::both().id_salt(id_salt).auto_shrink(false);
    if let Some(r) = tree.scroll_to_row.take() {
        area = area.vertical_scroll_offset(r as f32 * ROW_H);
    }
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        area.show_rows(ui, ROW_H, tree.rows.len(), |ui, range| {
            let width = ui.available_width().max(480.0);
            for ri in range {
                let Row { node: id, depth } = tree.rows[ri];
                let n = &tree.nodes[id as usize];
                let (rect, resp) = ui.allocate_exact_size(vec2(width, ROW_H), Sense::click());
                resp.widget_info(|| {
                    let state = match (n.child_count > 0, n.expanded) {
                        (false, _) => "",
                        (true, true) => ", expanded",
                        (true, false) => ", collapsed",
                    };
                    let info = WidgetInfo::labeled(egui::Role::TreeItem, true, format!("{} = {} {}{state}", n.name, n.ty, n.value));
                    info
                });
                let painter = ui.painter_at(rect);
                let selected = tree.selected == Some(id);
                if selected {
                    painter.rect_filled(rect, 0.0, p.current_line);
                } else if resp.hovered() {
                    painter.rect_filled(rect, 0.0, p.hover);
                }
                let x = rect.left() + 8.0 + depth as f32 * 16.0;
                if n.child_count > 0 {
                    painter.text(
                        egui::pos2(x + 6.0, rect.center().y),
                        Align2::CENTER_CENTER,
                        if n.expanded { ph::CARET_DOWN } else { ph::CARET_RIGHT },
                        theme::icon_font(12.0),
                        p.text_weak,
                    );
                }
                let mut job = LayoutJob::default();
                let fmt = |c: Color32| egui::TextFormat::simple(font.clone(), c);
                job.append(&n.name, 0.0, fmt(p.value.name));
                job.append(" = ", 0.0, fmt(p.text_weak));
                if matches!(n.kind, Kind::Array | Kind::Object) {
                    job.append(&format!("{{{}}} ", n.ty), 0.0, fmt(p.value.type_));
                }
                job.append(&n.value, 0.0, fmt(value_color(p, n.kind)));
                if n.changed {
                    job.append("   changed", 0.0, egui::TextFormat::simple(theme::body(size::SMALL), p.warning));
                }
                let galley = ui.fonts_mut(|f| f.layout_job(job));
                painter.galley(egui::pos2(x + 16.0, rect.center().y - galley.size().y / 2.0), galley, p.text);
                if resp.clicked() {
                    tree.selected = Some(id);
                    if n.child_count > 0 {
                        toggled = Some(ri);
                    }
                }
                resp.context_menu(|ui| {
                    if ui.button(theme::icon_job(ph::COPY, "Copy value", theme::body(size::BODY), p.text)).clicked() {
                        ui.ctx().copy_text(n.value.to_string());
                        ui.close();
                    }
                    if ui.button(theme::icon_job(ph::COPY, "Copy name", theme::body(size::BODY), p.text)).clicked() {
                        ui.ctx().copy_text(n.name.to_string());
                        ui.close();
                    }
                });
            }
        });
    });
    if let Some(ri) = toggled {
        tree.toggle_row(ri);
        changed = true;
    }
    // Ctrl+C copies the selected value, as in PhpStorm's Copy Value.
    if let Some(id) = tree.selected {
        if ui.input(|i| i.events.iter().any(|e| matches!(e, egui::Event::Copy))) && ui.memory(|m| m.focused().is_none()) {
            ui.ctx().copy_text(tree.nodes[id as usize].value.to_string());
        }
    }
    changed
}
