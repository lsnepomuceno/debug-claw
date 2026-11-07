//! The Feed and Debugger views from docs/ui.md, with mock data.

use std::time::Instant;

use eframe::egui::{
    self, Align, Align2, CornerRadius, Frame, Layout, Margin, RichText, Sense, Stroke, Ui, vec2,
};
use egui_phosphor::regular as ph;

use crate::actions::{self, ActionId, Style};
use crate::dumps::{self, Feed};
use crate::source::{self, Source};
use crate::theme::{self, DumpColor, size};
use crate::tree::{self, Builder, Kind, VarTree};

fn section(ui: &mut Ui, title: &str) {
    let p = theme::palette(ui.ctx());
    ui.add_space(10.0);
    ui.label(RichText::new(title.to_uppercase()).font(theme::semibold(11.0)).color(p.text_weak));
    ui.add_space(2.0);
}

/// A sidebar row: optional color swatch, name, count. Returns clicked.
fn filter_row(ui: &mut Ui, swatch: Option<egui::Color32>, name: &str, count: usize, selected: bool) -> bool {
    let p = theme::palette(ui.ctx());
    let (rect, r) = ui.allocate_exact_size(vec2(ui.available_width(), 26.0), Sense::click());
    r.widget_info(|| egui::WidgetInfo::selected(egui::Role::ListItem, true, selected, format!("{name} {count}")));
    let painter = ui.painter();
    if selected {
        painter.rect_filled(rect, size::RADIUS_SMALL, p.accent_soft);
    } else if r.hovered() {
        painter.rect_filled(rect, size::RADIUS_SMALL, p.hover);
    }
    let mut x = rect.left() + 8.0;
    if let Some(c) = swatch {
        painter.circle_filled(egui::pos2(x + 4.0, rect.center().y), 4.5, c);
        x += 16.0;
    }
    // Clip long names (request URIs) before the count.
    let name_rect = egui::Rect::from_min_max(rect.min, egui::pos2(rect.right() - 40.0, rect.bottom()));
    ui.painter_at(name_rect).text(egui::pos2(x, rect.center().y), Align2::LEFT_CENTER, name, theme::body(size::BODY), p.text);
    let painter = ui.painter();
    painter.text(rect.right_center() - vec2(8.0, 0.0), Align2::RIGHT_CENTER, count.to_string(), theme::mono(size::SMALL), p.text_weak);
    r.on_hover_text(format!("Show only {name}")).clicked()
}

pub struct FeedState {
    pub feed: Feed,
    pub paused: bool,
    pub clear_on_request: bool,
    pub filter: String,
}

pub fn feed_view(ui: &mut Ui, s: &mut FeedState) {
    let p = theme::palette(ui.ctx());
    egui::Panel::left("feed-sidebar")
        .exact_size(200.0)
        .resizable(false)
        .frame(Frame::new().fill(p.surface_alt).inner_margin(Margin::symmetric(10, 6)).stroke(Stroke::new(1.0, p.border)))
        .show(ui, |ui| {
            egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
                let mut changed = false;
                section(ui, "Screens");
                let screens = s.feed.counts(|d| d.screen);
                for name in ["default", "orders", "checkout"] {
                    let sel = s.feed.filter_screen == Some(name);
                    if filter_row(ui, None, name, *screens.get(name).unwrap_or(&0), sel) {
                        s.feed.filter_screen = if sel { None } else { Some(name) };
                        changed = true;
                    }
                }
                section(ui, "Colors");
                let colors = s.feed.counts(|d| d.color);
                for c in DumpColor::ALL {
                    let n = *colors.get(&c).unwrap_or(&0);
                    if n == 0 {
                        continue;
                    }
                    let sel = s.feed.filter_color == Some(c);
                    if filter_row(ui, Some(c.color()), c.name(), n, sel) {
                        s.feed.filter_color = if sel { None } else { Some(c) };
                        changed = true;
                    }
                }
                section(ui, "Labels");
                let labels = s.feed.counts(|d| d.label);
                let mut names: Vec<_> = labels.iter().collect();
                names.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
                for (name, n) in names {
                    let sel = s.feed.filter_label == Some(*name);
                    if filter_row(ui, None, name, *n, sel) {
                        s.feed.filter_label = if sel { None } else { Some(*name) };
                        changed = true;
                    }
                }
                section(ui, "Requests");
                let reqs = s.feed.counts(|d| d.request);
                let mut names: Vec<_> = reqs.iter().collect();
                names.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
                for (name, n) in names.into_iter().take(6) {
                    filter_row(ui, None, name, *n, false);
                }
                if changed {
                    s.feed.refilter();
                }
            });
        });
    egui::CentralPanel::no_frame().show(ui, |ui| {
        Frame::new().inner_margin(Margin { left: 14, right: 14, top: 10, bottom: 8 }).show(ui, |ui| {
            ui.horizontal(|ui| {
                actions::button(ui, ActionId::ClearFeed, Style::Labeled, None);
                let pause = actions::button(ui, ActionId::PauseFeed, Style::Labeled, None);
                if pause.clicked() {
                    s.paused = !s.paused;
                }
                let r = actions::button(ui, ActionId::ClearOnRequest, Style::Labeled, None);
                if r.clicked() {
                    s.clear_on_request = !s.clear_on_request;
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let edit = egui::TextEdit::singleline(&mut s.filter)
                        .hint_text(theme::icon_job(ph::FUNNEL, "Filter", theme::body(size::BODY), p.text_weak))
                        .desired_width(200.0);
                    ui.add(edit).on_hover_ui(|ui| actions::tooltip(ui, actions::get(ActionId::Filter), None));
                    ui.label(RichText::new(format!("{} of {} dumps", s.feed.shown(), s.feed.dumps.len())).color(p.text_weak).font(theme::body(size::SMALL)));
                });
            });
            ui.add_space(8.0);
            dumps::show(ui, &mut s.feed, "feed");
        });
    });
}

pub struct Debugger {
    pub source: Source,
    pub vars: VarTree,
    pub tab: usize,
    pub console_input: String,
    pub console: Vec<(String, String)>,
    pub frame_sel: usize,
    pub vendor_open: bool,
    pub started: Instant,
    pub live: bool,
}

const ORDER_SERVICE: &str = r#"<?php

declare(strict_types=1);

namespace App\Service;

use App\Domain\Order;
use App\Gateway\PaymentGateway;
use App\Repository\OrderRepository;
use App\Exception\OrderNotFound;
use Psr\Log\LoggerInterface;

/**
 * Charges orders through the configured payment gateway.
 */
final class OrderService
{
    private const MIN_TOTAL = 0.5;

    public function __construct(
        private readonly OrderRepository $orders,
        private readonly PaymentGateway $gateway,
        private readonly LoggerInterface $logger,
    ) {
    }

    public function find(int $id): Order
    {
        return $this->orders->find($id)
            ?? throw new OrderNotFound("Order {$id} not found");
    }

    /**
     * Charges the order total. Returns the gateway reference.
     */
    public function charge(int $id): string
    {
        $this->logger->info('Charging order', ['id' => $id]);

        $order = $this->orders->find($id);
        $total = $order->total;
        $this->gateway->charge($total);

        if ($total < self::MIN_TOTAL) {
            $this->logger->warning('Total below minimum', ['total' => $total]);
        }

        return $this->gateway->lastReference();
    }

    public function refund(Order $order, ?float $amount = null): void
    {
        $amount ??= $order->total;
        $this->gateway->refund($order->reference, $amount);
        $order->markRefunded($amount);
        $this->orders->save($order);
    }
}
"#;

fn locals() -> VarTree {
    let mut b = Builder::new();
    let order = b.add(None, "$order", Kind::Object, "#812", "App\\Order");
    let id = b.add(None, "$id", Kind::Num, "42", "int");
    let total = b.add(None, "$total", Kind::Num, "0.0", "float");
    b.add(None, "$this", Kind::Object, "#77", "App\\Service\\OrderService");
    let _ = id;
    b.changed(total);
    b.add(Some(order), "id", Kind::Num, "42", "int");
    let t = b.add(Some(order), "total", Kind::Num, "0.0", "float");
    b.changed(t);
    b.add(Some(order), "status", Kind::Str, "\"pending\"", "string");
    let customer = b.add(Some(order), "customer", Kind::Object, "#3", "App\\Customer");
    let items = b.add(Some(order), "items", Kind::Array, "", "array(3)");
    b.add(Some(order), "createdAt", Kind::Object, "2026-11-07 12:01:05", "DateTimeImmutable");
    b.add(Some(customer), "id", Kind::Num, "3", "int");
    b.add(Some(customer), "email", Kind::Str, "\"ana@example.com\"", "string");
    b.add(Some(customer), "vip", Kind::Bool, "false", "bool");
    for (k, sku) in ["SKU-1042", "SKU-2210", "SKU-0007"].iter().enumerate() {
        b.add(Some(items), &format!("[{k}]"), Kind::Object, &format!("#{}", 900 + k), "App\\OrderItem");
        let _ = sku;
    }
    b.expand(order);
    b.build()
}

impl Debugger {
    pub fn new(live: bool) -> Self {
        let mut source = Source::from_text("OrderService.php", ORDER_SERVICE.to_string());
        source.breakpoints.insert(42);
        source.breakpoints.insert(38);
        source.inline.insert(40, "$order: App\\Order #812".into());
        source.inline.insert(41, "$total: 0.0".into());
        source.jump_to(42);
        source.current = Some(42);
        Debugger {
            source,
            vars: locals(),
            tab: 0,
            console_input: String::new(),
            console: vec![("$order->items->count()".into(), "3".into()), ("$order->customer->email".into(), "\"ana@example.com\"".into())],
            frame_sel: 0,
            vendor_open: false,
            started: Instant::now(),
            live,
        }
    }
}

fn pill(ui: &mut Ui, icon: &str, text: &str, fg: egui::Color32, bg: egui::Color32) {
    Frame::new().fill(bg).corner_radius(10).inner_margin(Margin::symmetric(8, 2)).show(ui, |ui| {
        ui.label(theme::icon_job(icon, text, theme::medium(size::SMALL), fg));
    });
}

pub fn debugger_view(ui: &mut Ui, d: &mut Debugger) {
    let p = theme::palette(ui.ctx());
    // Session header and stepping toolbar.
    egui::Panel::top("dbg-header")
        .frame(Frame::new().fill(p.surface).inner_margin(Margin { left: 14, right: 14, top: 10, bottom: 10 }).stroke(Stroke::new(1.0, p.border)))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("POST /orders").font(theme::semibold(size::HEADING)));
                ui.label(RichText::new("paused at").color(p.text_weak));
                ui.label(RichText::new("OrderService.php:42").font(theme::mono(size::MONO)).color(p.accent));
                ui.add_space(6.0);
                pill(ui, ph::USER, "driven by: you", p.accent, p.accent_soft);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    actions::button(ui, ActionId::TakeOver, Style::Labeled, Some("You are already driving this session"));
                    let left = if d.live { 291u64.saturating_sub(d.started.elapsed().as_secs()) } else { 291 };
                    let t = format!("{}:{:02}", left / 60, left % 60);
                    ui.label(theme::icon_job(ph::TIMER, &t, theme::mono(size::MONO), p.text_weak))
                        .on_hover_text("Time until the paused worker is released");
                    if d.live {
                        ui.ctx().request_repaint_after(std::time::Duration::from_secs(1));
                    }
                });
            });
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                actions::button(ui, ActionId::Resume, Style::Primary, None);
                ui.add_space(6.0);
                actions::button(ui, ActionId::StepOver, Style::Icon, None);
                actions::button(ui, ActionId::StepInto, Style::Icon, None);
                actions::button(ui, ActionId::StepOut, Style::Icon, None);
                actions::button(ui, ActionId::RunToCursor, Style::Icon, Some("Place the text cursor on a line first"));
                ui.add_space(6.0);
                ui.separator();
                ui.add_space(6.0);
                actions::button(ui, ActionId::Stop, Style::Labeled, None);
                actions::button(ui, ActionId::Evaluate, Style::Labeled, None);
            });
        });

    // Console at the very bottom, variables above it.
    egui::Panel::bottom("dbg-console")
        .exact_size(124.0)
        .frame(Frame::new().fill(p.surface).inner_margin(Margin::symmetric(14, 8)).stroke(Stroke::new(1.0, p.border)))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(theme::icon_job(ph::TERMINAL, "Console", theme::semibold(size::BODY), p.text));
                ui.label(RichText::new("runs in the paused frame").color(p.text_weak).font(theme::body(size::SMALL)));
            });
            for (expr, out) in d.console.iter() {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(">>>").font(theme::mono(size::MONO)).color(p.text_weak));
                    ui.add(egui::Label::new(RichText::new(expr).font(theme::mono(size::MONO))).selectable(true));
                    ui.label(theme::icon_job(ph::ARROW_RIGHT, out, theme::mono(size::MONO), p.value.number));
                });
            }
            ui.horizontal(|ui| {
                ui.label(RichText::new(">>>").font(theme::mono(size::MONO)).color(p.accent));
                let r = ui.add(egui::TextEdit::singleline(&mut d.console_input).font(theme::mono(size::MONO)).hint_text("PHP expression").desired_width(ui.available_width() - 120.0));
                let submit = actions::button(ui, ActionId::Evaluate, Style::Labeled, None).clicked() || (r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)));
                if submit && !d.console_input.trim().is_empty() {
                    let e = std::mem::take(&mut d.console_input);
                    d.console.push((e, "null".into()));
                    if d.console.len() > 2 {
                        d.console.remove(0);
                    }
                }
            });
        });

    egui::Panel::bottom("dbg-vars")
        .exact_size(250.0)
        .frame(Frame::new().fill(p.surface).inner_margin(Margin::symmetric(10, 6)).stroke(Stroke::new(1.0, p.border)))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                for (i, t) in ["Locals", "Superglobals", "Constants", "Watches"].iter().enumerate() {
                    let sel = d.tab == i;
                    let text = RichText::new(*t).font(if sel { theme::semibold(size::BODY) } else { theme::body(size::BODY) }).color(if sel { p.text } else { p.text_weak });
                    let r = ui.add(egui::Button::new(text).frame(false).min_size(vec2(0.0, 26.0)));
                    if sel {
                        ui.painter().hline(r.rect.x_range(), r.rect.bottom() + 2.0, Stroke::new(2.0, p.accent));
                    }
                    if r.clicked() {
                        d.tab = i;
                    }
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(theme::icon_job(ph::DOT_OUTLINE, "changed since last step", theme::body(size::SMALL), p.warning));
                });
            });
            ui.add_space(4.0);
            if d.tab == 0 {
                tree::show(ui, &mut d.vars, "locals");
            } else {
                ui.add_space(20.0);
                ui.vertical_centered(|ui| {
                    ui.label(RichText::new("Not filled in this mockup").color(p.text_weak));
                });
            }
        });

    egui::Panel::left("dbg-frames")
        .exact_size(250.0)
        .resizable(false)
        .frame(Frame::new().fill(p.surface_alt).inner_margin(Margin::symmetric(8, 8)).stroke(Stroke::new(1.0, p.border)))
        .show(ui, |ui| {
            ui.label(RichText::new("FRAMES").font(theme::semibold(11.0)).color(p.text_weak));
            ui.add_space(4.0);
            let frames = [("OrderService::charge", "OrderService.php:42"), ("OrderController::store", "OrderController.php:27")];
            for (i, (f, at)) in frames.iter().enumerate() {
                let sel = d.frame_sel == i;
                let (rect, r) = ui.allocate_exact_size(vec2(ui.available_width(), 40.0), Sense::click());
                r.widget_info(|| egui::WidgetInfo::selected(egui::Role::ListItem, true, sel, *f));
                let painter = ui.painter();
                if sel {
                    painter.rect_filled(rect, CornerRadius::same(size::RADIUS_SMALL), p.accent_soft);
                } else if r.hovered() {
                    painter.rect_filled(rect, CornerRadius::same(size::RADIUS_SMALL), p.hover);
                }
                painter.text(rect.left_top() + vec2(10.0, 11.0), Align2::LEFT_CENTER, *f, theme::medium(size::BODY), p.text);
                painter.text(rect.left_top() + vec2(10.0, 29.0), Align2::LEFT_CENTER, *at, theme::mono(size::SMALL), p.text_weak);
                if r.clicked() {
                    d.frame_sel = i;
                }
            }
            let label = theme::icon_job(if d.vendor_open { ph::CARET_DOWN } else { ph::CARET_RIGHT }, "vendor, 14 frames", theme::body(size::BODY), p.text_weak);
            if ui.add(egui::Button::new(label).frame(false)).on_hover_text("Show the framework frames").clicked() {
                d.vendor_open = !d.vendor_open;
            }
            if d.vendor_open {
                for f in ["Kernel::handle", "Router::dispatch", "Pipeline::then"] {
                    ui.label(RichText::new(format!("    {f}")).font(theme::mono(size::SMALL)).color(p.text_weak));
                }
            }
            let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 40.0), Sense::hover());
            ui.painter().text(rect.left_top() + vec2(10.0, 11.0), Align2::LEFT_CENTER, "{main}", theme::medium(size::BODY), p.text);
            ui.painter().text(rect.left_top() + vec2(10.0, 29.0), Align2::LEFT_CENTER, "public/index.php:12", theme::mono(size::SMALL), p.text_weak);
        });

    egui::CentralPanel::no_frame().show(ui, |ui| {
        Frame::new().fill(p.surface).inner_margin(Margin::symmetric(0, 6)).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.add_space(12.0);
                ui.label(theme::icon_job(ph::FILE_CODE, &d.source.name, theme::medium(size::BODY), p.text));
                ui.label(RichText::new("src/Service").color(p.text_weak).font(theme::body(size::SMALL)));
            });
            ui.add_space(4.0);
            source::show(ui, &mut d.source, "dbg-source");
        });
    });
}
