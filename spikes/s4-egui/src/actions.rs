//! The action registry from docs/ui.md principle 1: each action has a
//! name, a description, an icon, a shortcut and the MCP tool behind it.
//! Buttons and tooltips are built from it, so they cannot drift apart.

use eframe::egui::{self, Button, Response, RichText, Sense, Ui, vec2};
use egui_phosphor::regular as ph;

use crate::theme::{self, size};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ActionId {
    Resume,
    StepOver,
    StepInto,
    StepOut,
    RunToCursor,
    Stop,
    Evaluate,
    TakeOver,
    ClearFeed,
    PauseFeed,
    ClearOnRequest,
    Filter,
    CopyDump,
    PinDump,
    OpenInEditor,
    HideSimilar,
    Search,
    AlwaysOnTop,
    ToggleTheme,
}

pub struct Action {
    pub id: ActionId,
    pub name: &'static str,
    pub description: &'static str,
    pub icon: &'static str,
    pub shortcut: Option<&'static str>,
    /// None for actions that only change the app itself (theme, pin).
    pub mcp: Option<&'static str>,
}

pub const ACTIONS: &[Action] = &[
    Action { id: ActionId::Resume, name: "Resume", description: "Runs the request until the next breakpoint or the end.", icon: ph::PLAY, shortcut: Some("F9"), mcp: Some("continue") },
    Action { id: ActionId::StepOver, name: "Step over", description: "Runs the current line and stops at the next one in this frame.", icon: ph::ARROW_BEND_DOWN_RIGHT, shortcut: Some("F8"), mcp: Some("step") },
    Action { id: ActionId::StepInto, name: "Step into", description: "Enters the function called on the current line.", icon: ph::ARROW_ELBOW_DOWN_RIGHT, shortcut: Some("F7"), mcp: Some("step") },
    Action { id: ActionId::StepOut, name: "Step out", description: "Runs until the current function returns.", icon: ph::ARROW_U_UP_LEFT, shortcut: Some("Shift+F8"), mcp: Some("step") },
    Action { id: ActionId::RunToCursor, name: "Run to cursor", description: "Runs until the line under the text cursor.", icon: ph::CURSOR_TEXT, shortcut: Some("Alt+F9"), mcp: Some("run_to") },
    Action { id: ActionId::Stop, name: "Stop", description: "Ends the session and lets the request finish without pausing again.", icon: ph::STOP, shortcut: Some("Ctrl+F2"), mcp: Some("stop") },
    Action { id: ActionId::Evaluate, name: "Evaluate", description: "Runs a PHP expression in the paused frame.", icon: ph::TERMINAL, shortcut: Some("Alt+F8"), mcp: Some("evaluate") },
    Action { id: ActionId::TakeOver, name: "Take over", description: "Takes control of the session from the agent.", icon: ph::HAND_POINTING, shortcut: None, mcp: Some("session_driver") },
    Action { id: ActionId::ClearFeed, name: "Clear", description: "Removes every dump from the feed.", icon: ph::TRASH, shortcut: Some("Ctrl+K"), mcp: Some("clear_dumps") },
    Action { id: ActionId::PauseFeed, name: "Pause feed", description: "Stops showing new dumps until resumed. They are still stored.", icon: ph::PAUSE, shortcut: None, mcp: None },
    Action { id: ActionId::ClearOnRequest, name: "Clear on new request", description: "Clears the feed each time a new request starts.", icon: ph::ARROW_COUNTER_CLOCKWISE, shortcut: None, mcp: None },
    Action { id: ActionId::Filter, name: "Filter", description: "Shows only dumps that match the text.", icon: ph::FUNNEL, shortcut: Some("Ctrl+F"), mcp: Some("list_dumps") },
    Action { id: ActionId::CopyDump, name: "Copy", description: "Copies the dump as text.", icon: ph::COPY, shortcut: Some("Ctrl+C"), mcp: Some("get_dump") },
    Action { id: ActionId::PinDump, name: "Pin", description: "Keeps the dump at the top of the feed.", icon: ph::PUSH_PIN, shortcut: None, mcp: None },
    Action { id: ActionId::OpenInEditor, name: "Open in editor", description: "Opens the line that produced the dump.", icon: ph::ARROW_SQUARE_OUT, shortcut: None, mcp: None },
    Action { id: ActionId::HideSimilar, name: "Hide similar", description: "Hides dumps from the same line.", icon: ph::EYE_SLASH, shortcut: None, mcp: None },
    Action { id: ActionId::Search, name: "Search", description: "Searches dumps, requests and variables.", icon: ph::MAGNIFYING_GLASS, shortcut: Some("Ctrl+Shift+F"), mcp: Some("search") },
    Action { id: ActionId::AlwaysOnTop, name: "Always on top", description: "Keeps the window above other windows.", icon: ph::PUSH_PIN, shortcut: None, mcp: None },
    Action { id: ActionId::ToggleTheme, name: "Theme", description: "Switches between the light and dark themes.", icon: ph::MOON, shortcut: None, mcp: None },
];

pub fn get(id: ActionId) -> &'static Action {
    ACTIONS.iter().find(|a| a.id == id).expect("every ActionId is registered")
}

/// The rich tooltip from docs/ui.md principle 3.
pub fn tooltip(ui: &mut Ui, a: &Action, disabled_reason: Option<&str>) {
    let p = theme::palette(ui.ctx());
    ui.set_max_width(280.0);
    ui.horizontal(|ui| {
        ui.label(RichText::new(a.name).font(theme::semibold(size::BODY)));
        if let Some(k) = a.shortcut {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                kbd(ui, k);
            });
        }
    });
    ui.label(RichText::new(a.description).color(p.text_weak));
    if let Some(tool) = a.mcp {
        ui.add_space(2.0);
        ui.label(
            RichText::new(format!("MCP: {tool}"))
                .font(theme::mono(size::SMALL))
                .color(p.accent),
        );
    }
    if let Some(why) = disabled_reason {
        ui.add_space(2.0);
        ui.label(
            theme::icon_job(egui_phosphor::regular::PROHIBIT, why, theme::body(size::BODY), p.warning),
        );
    }
}

pub fn kbd(ui: &mut Ui, k: &str) {
    let p = theme::palette(ui.ctx());
    egui::Frame::new()
        .fill(p.surface_alt)
        .stroke(egui::Stroke::new(1.0, p.border))
        .corner_radius(3)
        .inner_margin(egui::Margin::symmetric(5, 1))
        .show(ui, |ui| {
            ui.label(RichText::new(k).font(theme::mono(11.0)).color(p.text_weak));
        });
}

pub enum Style {
    /// Icon and label, the default for main actions.
    Labeled,
    /// Icon only, where space is tight (stepping toolbar).
    Icon,
    /// Accent filled, for the one primary action in a bar.
    Primary,
    /// Icon only, no frame (card hover actions).
    Ghost,
}

/// A button for an action, with the registry tooltip. `disabled_reason`
/// disables it and keeps the tooltip, which then says why.
pub fn button(ui: &mut Ui, id: ActionId, style: Style, disabled_reason: Option<&str>) -> Response {
    let a = get(id);
    let p = theme::palette(ui.ctx());
    let enabled = disabled_reason.is_none();
    let icon = RichText::new(a.icon).font(theme::icon_font(15.0));
    let btn = match style {
        Style::Labeled => Button::new((icon, RichText::new(a.name))).min_size(vec2(0.0, 28.0)),
        Style::Primary => Button::new((icon.color(p.on_accent), RichText::new(a.name).color(p.on_accent)))
        .fill(p.accent)
        .stroke(egui::Stroke::NONE)
        .min_size(vec2(0.0, 28.0)),
        Style::Icon => Button::new(icon).min_size(vec2(30.0, 28.0)),
        Style::Ghost => Button::new(icon.size(14.0)).frame(false).min_size(vec2(24.0, 22.0)),
    };
    let r = ui.add_enabled(enabled, btn);
    // The visible text starts with a private use codepoint (the icon), so
    // give screen readers the action name instead.
    r.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, enabled, a.name));
    let r = if enabled {
        r.on_hover_ui(|ui| tooltip(ui, a, None))
    } else {
        r.on_disabled_hover_ui(|ui| tooltip(ui, a, disabled_reason))
    };
    crate::app::record_rect(ui.ctx(), id, r.rect);
    r
}

/// A clickable row in the rail or a sidebar list.
pub fn nav_item(ui: &mut Ui, icon: &str, label: &str, selected: bool, badge: Option<&str>) -> Response {
    let p = theme::palette(ui.ctx());
    let (rect, r) = ui.allocate_exact_size(vec2(ui.available_width(), 30.0), Sense::click());
    r.widget_info(|| egui::WidgetInfo::selected(egui::Role::ListItem, true, selected, label));
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        if selected {
            painter.rect_filled(rect, size::RADIUS_SMALL, p.accent_soft);
        } else if r.hovered() {
            painter.rect_filled(rect, size::RADIUS_SMALL, p.hover);
        }
        let fg = if selected { p.accent } else { p.text };
        painter.text(
            rect.left_center() + vec2(10.0, 0.0),
            egui::Align2::LEFT_CENTER,
            icon,
            theme::icon_font(16.0),
            fg,
        );
        painter.text(
            rect.left_center() + vec2(34.0, 0.0),
            egui::Align2::LEFT_CENTER,
            label,
            if selected { theme::medium(size::BODY) } else { theme::body(size::BODY) },
            if selected { p.text } else { p.text },
        );
        if let Some(b) = badge {
            painter.text(
                rect.right_center() - vec2(10.0, 0.0),
                egui::Align2::RIGHT_CENTER,
                b,
                theme::body(size::SMALL),
                p.text_weak,
            );
        }
    }
    r
}
