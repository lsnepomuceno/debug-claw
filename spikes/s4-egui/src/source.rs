//! A PHP source view: tree-sitter highlighting computed once when the
//! file loads and kept as compact spans per line, then only the visible
//! rows are laid out. The gutter shows line numbers and breakpoints, and
//! Ctrl+G opens a jump-to-line field.

use std::collections::{HashMap, HashSet};
use std::time::Instant;

use eframe::egui::{
    self, Align2, Color32, Key, Label, Rect, RichText, ScrollArea, Sense, Stroke, TextFormat, Ui,
    pos2, text::LayoutJob, vec2,
};
use tree_sitter_highlight::{HighlightConfiguration, HighlightEvent, Highlighter};

use crate::theme::{self, size};

const NAMES: [&str; 19] = [
    "comment", "constant", "constant.builtin", "constructor", "function", "function.builtin",
    "function.method", "keyword", "module", "number", "operator", "property", "string", "tag",
    "type", "type.builtin", "variable", "variable.builtin", "attribute",
];
const PLAIN: u8 = u8::MAX;

#[derive(Clone, Copy)]
struct Span {
    start: u32,
    end: u32,
    class: u8,
}

pub struct Source {
    pub name: String,
    text: String,
    /// Byte offset where each line starts, plus the text length.
    starts: Vec<u32>,
    spans: Vec<Vec<Span>>,
    pub breakpoints: HashSet<u32>,
    pub current: Option<u32>,
    pub inline: HashMap<u32, String>,
    pub highlight_ms: f64,
    goto_open: bool,
    goto_text: String,
    scroll_to: Option<u32>,
    pub first_line: u32,
}

pub const ROW_H: f32 = 19.0;

impl Source {
    pub fn from_text(name: &str, text: String) -> Self {
        let mut starts = vec![0u32];
        for (i, b) in text.bytes().enumerate() {
            if b == b'\n' {
                starts.push(i as u32 + 1);
            }
        }
        if *starts.last().unwrap() as usize != text.len() {
            starts.push(text.len() as u32);
        }
        let t = Instant::now();
        let spans = highlight(&text, &starts);
        let highlight_ms = t.elapsed().as_secs_f64() * 1000.0;
        Source {
            name: name.into(),
            text,
            starts,
            spans,
            breakpoints: HashSet::new(),
            current: None,
            inline: HashMap::new(),
            highlight_ms,
            goto_open: false,
            goto_text: String::new(),
            scroll_to: None,
            first_line: 1,
        }
    }

    pub fn generated(lines: usize) -> Self {
        Self::from_text("LargeOrderService.php", generate(lines))
    }

    pub fn lines(&self) -> usize {
        self.starts.len() - 1
    }

    fn line(&self, i: usize) -> &str {
        let a = self.starts[i] as usize;
        let b = self.starts[i + 1] as usize;
        self.text[a..b].trim_end_matches(['\n', '\r'])
    }

    pub fn open_goto(&mut self) {
        self.goto_open = true;
        self.goto_text.clear();
    }

    /// Scroll so that 1-based `line` is in view.
    pub fn jump_to(&mut self, line: u32) {
        self.scroll_to = Some(line.clamp(1, self.lines() as u32));
    }
}

fn highlight(text: &str, starts: &[u32]) -> Vec<Vec<Span>> {
    let mut out = vec![Vec::new(); starts.len() - 1];
    let mut cfg = match HighlightConfiguration::new(
        tree_sitter_php::LANGUAGE_PHP.into(),
        "php",
        tree_sitter_php::HIGHLIGHTS_QUERY,
        tree_sitter_php::INJECTIONS_QUERY,
        "",
    ) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("highlight query error: {e}");
            return out;
        }
    };
    cfg.configure(&NAMES);
    let mut hl = Highlighter::new();
    let Ok(events) = hl.highlight(&cfg, text.as_bytes(), None, None, |_| None) else {
        return out;
    };
    let mut stack: Vec<u8> = Vec::new();
    for ev in events.flatten() {
        match ev {
            HighlightEvent::HighlightStart(h) => stack.push(h.0 as u8),
            HighlightEvent::HighlightEnd => {
                stack.pop();
            }
            HighlightEvent::Source { start, end } => {
                let class = *stack.last().unwrap_or(&PLAIN);
                if class == PLAIN {
                    continue;
                }
                // Split the range at line boundaries.
                let mut s = start as u32;
                let e = end as u32;
                while s < e {
                    let line = starts.partition_point(|&x| x <= s) - 1;
                    let line_end = starts[line + 1].min(e);
                    let base = starts[line];
                    out[line].push(Span { start: s - base, end: line_end - base, class });
                    s = line_end;
                }
            }
        }
    }
    out
}

fn class_color(p: &theme::Palette, class: u8) -> Color32 {
    let s = &p.syntax;
    match NAMES.get(class as usize).copied() {
        Some("comment") => s.comment,
        Some("constant" | "constant.builtin") => s.constant,
        Some("function" | "function.builtin" | "function.method" | "constructor") => s.function,
        Some("keyword" | "tag" | "module" | "module.builtin") => s.keyword,
        Some("number") => s.number,
        Some("string") => s.string,
        Some("type" | "type.builtin") => s.type_,
        Some("variable" | "variable.builtin" | "property") => s.variable,
        Some("attribute") => s.attribute,
        Some("operator") => s.operator,
        _ => s.plain,
    }
}

fn line_job(p: &theme::Palette, text: &str, spans: &[Span], font: &egui::FontId) -> LayoutJob {
    let mut job = LayoutJob::default();
    let fmt = |c: Color32| TextFormat { font_id: font.clone(), color: c, ..Default::default() };
    let mut at = 0usize;
    let len = text.len();
    for sp in spans {
        let (a, b) = ((sp.start as usize).min(len), (sp.end as usize).min(len));
        if a > at {
            job.append(&text[at..a], 0.0, fmt(p.syntax.plain));
        }
        if b > a.max(at) {
            job.append(&text[a.max(at)..b], 0.0, fmt(class_color(p, sp.class)));
        }
        at = at.max(b);
    }
    if at < len {
        job.append(&text[at..], 0.0, fmt(p.syntax.plain));
    }
    job
}

/// The virtualised source view. Returns the line whose gutter was
/// clicked, if any, after toggling its breakpoint.
pub fn show(ui: &mut Ui, src: &mut Source, id_salt: &str) {
    let p = theme::palette(ui.ctx());
    let font = theme::mono(size::MONO);
    if ui.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, Key::G)) {
        src.goto_open = true;
        src.goto_text.clear();
    }
    if src.goto_open {
        ui.horizontal(|ui| {
            ui.label(RichText::new("Go to line").color(p.text_weak));
            let hint = format!("1..{}", src.lines());
            let r = ui.add(egui::TextEdit::singleline(&mut src.goto_text).desired_width(80.0).hint_text(hint));
            r.request_focus();
            if r.lost_focus() || ui.input(|i| i.key_pressed(Key::Enter)) {
                if let Ok(n) = src.goto_text.trim().parse::<u32>() {
                    src.jump_to(n);
                }
                src.goto_open = false;
            }
            if ui.input(|i| i.key_pressed(Key::Escape)) {
                src.goto_open = false;
            }
        });
    }
    let digits = (src.lines() as f32 + src.first_line as f32).log10().floor() as usize + 1;
    let char_w = ui.fonts_mut(|f| f.glyph_width(&font, '0'));
    let gutter = 22.0 + digits as f32 * char_w + 14.0;
    let total = src.lines();
    let mut area = ScrollArea::both().id_salt(id_salt).auto_shrink(false);
    if let Some(line) = src.scroll_to.take() {
        // Keep the target a third of the way down.
        let h = ui.available_height();
        area = area.vertical_scroll_offset(((line as f32 - 1.0) * ROW_H - h / 3.0).max(0.0));
        src.current.get_or_insert(line);
    }
    ui.spacing_mut().item_spacing.y = 0.0;
    let mut toggle: Option<u32> = None;
    area.show_rows(ui, ROW_H, total, |ui, range| {
        for i in range {
            let n = i as u32 + src.first_line;
            let (row, _) = ui.allocate_exact_size(vec2(ui.available_width().max(900.0), ROW_H), Sense::hover());
            let painter = ui.painter();
            if src.current == Some(n) {
                painter.rect_filled(row, 0, p.current_line);
            }
            let g = Rect::from_min_size(row.min, vec2(gutter, ROW_H));
            let gr = ui.interact(g, ui.scope_id().with(("gutter", n)), Sense::click());
            let has_bp = src.breakpoints.contains(&n);
            if has_bp || gr.hovered() {
                let c = Color32::from_rgb(0xE5, 0x48, 0x4D);
                painter.circle_filled(pos2(g.left() + 11.0, g.center().y), 5.0, if has_bp { c } else { c.gamma_multiply(0.35) });
            }
            if gr.clicked() {
                toggle = Some(n);
            }
            gr.on_hover_text(if has_bp { "Remove breakpoint" } else { "Add breakpoint" });
            let num_color = if src.current == Some(n) { p.text } else { p.text_weak };
            painter.text(pos2(g.right() - 12.0, g.center().y), Align2::RIGHT_CENTER, n.to_string(), font.clone(), num_color);
            if src.current == Some(n) {
                painter.text(pos2(g.right() - 6.0, g.center().y), Align2::CENTER_CENTER, egui_phosphor::regular::CARET_RIGHT, theme::icon_font(11.0), p.accent);
            }
            let text = src.line(i);
            let job = line_job(p, text, &src.spans[i], &font);
            let code = Rect::from_min_max(pos2(g.right() + 4.0, row.top()), row.max);
            let lr = crate::dumps::put_left(ui, code, Label::new(job).selectable(true).extend());
            if let Some(v) = src.inline.get(&n) {
                let x = lr.rect.left() + ui.fonts_mut(|f| f.glyph_width(&font, 'm')) * text.chars().count() as f32 + 24.0;
                let galley = ui.fonts_mut(|f| f.layout_no_wrap(v.clone(), font.clone(), p.text_weak));
                let pill = Rect::from_min_size(pos2(x - 6.0, row.top() + 2.0), vec2(galley.size().x + 12.0, ROW_H - 4.0));
                ui.painter().rect(pill, 3, p.accent_soft, Stroke::NONE, egui::StrokeKind::Inside);
                ui.painter().galley(pos2(x, row.center().y - galley.size().y / 2.0), galley, p.text_weak);
            }
        }
    });
    if let Some(n) = toggle
        && !src.breakpoints.remove(&n)
    {
        src.breakpoints.insert(n);
    }
}

/// A generated PHP file of about `lines` lines: a class whose methods
/// repeat a few shapes with different names and values.
pub fn generate(lines: usize) -> String {
    let mut s = String::with_capacity(lines * 48);
    s.push_str("<?php\n\ndeclare(strict_types=1);\n\nnamespace App\\Service;\n\nuse App\\Domain\\Order;\nuse App\\Repository\\OrderRepository;\nuse Psr\\Log\\LoggerInterface;\n\n/**\n * Generated for the S4 source view benchmark.\n */\nfinal class LargeOrderService\n{\n    private const MAX_ITEMS = 50;\n\n    public function __construct(\n        private readonly OrderRepository $orders,\n        private readonly LoggerInterface $logger,\n    ) {\n    }\n");
    let mut k = 0usize;
    let mut count = s.bytes().filter(|&b| b == b'\n').count();
    while count + 18 < lines {
        let h = crate::tree::mix(k as u64);
        let m = ["charge", "refund", "recalculate", "validate", "ship", "archive"][(h % 6) as usize];
        s.push_str(&format!(
            "\n    /**\n     * Step {k}: {m} the order when the total is above {lim}.\n     */\n    public function {m}{k}(int $id, float $rate = {rate}.5): ?array\n    {{\n        $order = $this->orders->find($id);\n        if ($order === null || count($order->items) > self::MAX_ITEMS) {{\n            return null;\n        }}\n        $total = 0.0;\n        foreach ($order->items as $i => $item) {{\n            $total += $item->price * $item->qty * (1 + $rate / 100);\n        }}\n        $this->logger->info(\"{m} {{id}}\", ['id' => $id, 'total' => $total]);\n        return ['id' => $id, 'total' => round($total, 2), 'step' => {k}];\n    }}\n",
            lim = 100 + h % 900,
            rate = h % 20,
        ));
        count += 17;
        k += 1;
    }
    s.push_str("}\n");
    s
}
