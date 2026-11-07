//! Visual tokens, fonts and the two themes. Every view reads its colors
//! and sizes from here, which is how docs/ui.md asks for named tokens.

use eframe::egui::{
    self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Margin, Shadow,
    Stroke, TextStyle, Theme, Visuals, epaint::text::FontTweak,
};

/// Spacing, radius and type sizes, named once.
pub mod size {
    pub const BODY: f32 = 13.5;
    pub const SMALL: f32 = 12.0;
    pub const HEADING: f32 = 15.0;
    pub const MONO: f32 = 12.5;
    pub const RADIUS: u8 = 6;
    pub const RADIUS_SMALL: u8 = 4;
    pub const GAP: f32 = 8.0;
    pub const PAD: i8 = 10;
}

/// Colors that egui's Visuals do not cover, per theme.
#[derive(Clone, Copy)]
pub struct Palette {
    pub dark: bool,
    pub bg: Color32,
    pub surface: Color32,
    pub surface_alt: Color32,
    pub border: Color32,
    pub text: Color32,
    pub text_weak: Color32,
    pub accent: Color32,
    pub accent_soft: Color32,
    pub on_accent: Color32,
    pub hover: Color32,
    pub current_line: Color32,
    pub success: Color32,
    pub warning: Color32,
    pub syntax: Syntax,
    pub value: ValueColors,
}

#[derive(Clone, Copy)]
pub struct Syntax {
    pub keyword: Color32,
    pub string: Color32,
    pub number: Color32,
    pub comment: Color32,
    pub variable: Color32,
    pub function: Color32,
    pub type_: Color32,
    pub constant: Color32,
    pub attribute: Color32,
    pub operator: Color32,
    pub plain: Color32,
}

#[derive(Clone, Copy)]
pub struct ValueColors {
    pub string: Color32,
    pub number: Color32,
    pub keyword: Color32,
    pub type_: Color32,
    pub name: Color32,
}

/// The dump colors from docs/ui.md (Radix Colors step 9, MIT).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum DumpColor {
    Red,
    Orange,
    Yellow,
    Green,
    Blue,
    Purple,
    Gray,
}

impl DumpColor {
    pub const ALL: [DumpColor; 7] = [
        DumpColor::Red,
        DumpColor::Orange,
        DumpColor::Yellow,
        DumpColor::Green,
        DumpColor::Blue,
        DumpColor::Purple,
        DumpColor::Gray,
    ];

    pub fn color(self) -> Color32 {
        match self {
            DumpColor::Red => Color32::from_rgb(0xE5, 0x48, 0x4D),
            DumpColor::Orange => Color32::from_rgb(0xF7, 0x6B, 0x15),
            DumpColor::Yellow => Color32::from_rgb(0xE2, 0xA3, 0x36),
            DumpColor::Green => Color32::from_rgb(0x30, 0xA4, 0x6C),
            DumpColor::Blue => Color32::from_rgb(0x00, 0x90, 0xFF),
            DumpColor::Purple => Color32::from_rgb(0x8E, 0x4E, 0xC6),
            DumpColor::Gray => Color32::from_rgb(0x8B, 0x8D, 0x98),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            DumpColor::Red => "red",
            DumpColor::Orange => "orange",
            DumpColor::Yellow => "yellow",
            DumpColor::Green => "green",
            DumpColor::Blue => "blue",
            DumpColor::Purple => "purple",
            DumpColor::Gray => "gray",
        }
    }
}

const fn rgb(hex: u32) -> Color32 {
    Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

pub const DARK: Palette = Palette {
    dark: true,
    bg: rgb(0x16171B),
    surface: rgb(0x1E1F24),
    surface_alt: rgb(0x25262C),
    border: rgb(0x2E3037),
    text: rgb(0xE4E5E9),
    text_weak: rgb(0x9A9DA7),
    accent: rgb(0x5B8CFF),
    accent_soft: Color32::from_rgba_premultiplied(0x16, 0x23, 0x40, 0x60),
    on_accent: rgb(0xFFFFFF),
    hover: rgb(0x2A2C33),
    current_line: rgb(0x2B3245),
    success: rgb(0x3DD68C),
    warning: rgb(0xFFB224),
    syntax: Syntax {
        keyword: rgb(0xCF8E6D),
        string: rgb(0x6AAB73),
        number: rgb(0x2AACB8),
        comment: rgb(0x7A7E85),
        variable: rgb(0xC77DBB),
        function: rgb(0x56A8F5),
        type_: rgb(0xD5B778),
        constant: rgb(0xC77DBB),
        attribute: rgb(0xB3AE60),
        operator: rgb(0xBCBEC4),
        plain: rgb(0xBCBEC4),
    },
    value: ValueColors {
        string: rgb(0x6AAB73),
        number: rgb(0x2AACB8),
        keyword: rgb(0xCF8E6D),
        type_: rgb(0x8C8F99),
        name: rgb(0xC77DBB),
    },
};

pub const LIGHT: Palette = Palette {
    dark: false,
    bg: rgb(0xF4F5F7),
    surface: rgb(0xFFFFFF),
    surface_alt: rgb(0xF8F9FA),
    border: rgb(0xE1E3E8),
    text: rgb(0x1C1E23),
    text_weak: rgb(0x6A6E78),
    accent: rgb(0x2F6BFF),
    accent_soft: rgb(0xE8EFFF),
    on_accent: rgb(0xFFFFFF),
    hover: rgb(0xEEF0F3),
    current_line: rgb(0xE4ECFF),
    success: rgb(0x18794E),
    warning: rgb(0xAD5700),
    syntax: Syntax {
        keyword: rgb(0x0033B3),
        string: rgb(0x067D17),
        number: rgb(0x1750EB),
        comment: rgb(0x8C8C8C),
        variable: rgb(0x871094),
        function: rgb(0x00627A),
        type_: rgb(0x000000),
        constant: rgb(0x871094),
        attribute: rgb(0x9E880D),
        operator: rgb(0x1C1E23),
        plain: rgb(0x1C1E23),
    },
    value: ValueColors {
        string: rgb(0x067D17),
        number: rgb(0x1750EB),
        keyword: rgb(0x0033B3),
        type_: rgb(0x6A6E78),
        name: rgb(0x871094),
    },
};

pub fn palette(ctx: &egui::Context) -> &'static Palette {
    if ctx.global_style().visuals.dark_mode { &DARK } else { &LIGHT }
}

pub fn mono(size: f32) -> FontId {
    FontId::new(size, FontFamily::Monospace)
}

pub fn body(size: f32) -> FontId {
    FontId::new(size, FontFamily::Proportional)
}

pub fn medium(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("medium".into()))
}

pub fn semibold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("semibold".into()))
}

pub fn icon_font(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("icons".into()))
}

/// An icon followed by text. Inter defines 745 glyphs in the private use
/// area where Phosphor puts its icons, so an icon inside a Proportional
/// string would render Inter's glyph. The icon gets the "icons" family
/// (Phosphor first) in its own section instead.
pub fn icon_job(icon: &str, text: &str, font: FontId, color: Color32) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    let fmt = |font_id: FontId| egui::TextFormat { font_id, color, valign: egui::Align::Center, ..Default::default() };
    job.append(icon, 0.0, fmt(icon_font(font.size + 1.0)));
    job.append(text, font.size * 0.5, fmt(font));
    job
}

/// Inter and JetBrains Mono from S4_FONT_DIR (fetched by the Dockerfile),
/// with Phosphor as a fallback in every family. Icons next to Inter text
/// go through `icon_job`.
/// Missing files fall back to egui's bundled fonts, and the result says
/// which fonts were found.
pub fn install_fonts(ctx: &egui::Context) -> String {
    let dir = std::env::var("S4_FONT_DIR").unwrap_or_else(|_| "/opt/fonts".into());
    let mut fonts = FontDefinitions::default();
    let mut found = Vec::new();
    let mut load = |file: &str, key: &str, tweak: FontTweak| {
        match std::fs::read(format!("{dir}/{file}")) {
            Ok(bytes) => {
                fonts
                    .font_data
                    .insert(key.into(), FontData::from_owned(bytes).tweak(tweak).into());
                found.push(file.to_owned());
                true
            }
            Err(_) => false,
        }
    };
    let plain = FontTweak::default();
    let inter = load("Inter-Regular.ttf", "inter", plain.clone());
    let inter_medium = load("Inter-Medium.ttf", "inter-medium", plain.clone());
    let inter_semibold = load("Inter-SemiBold.ttf", "inter-semibold", plain.clone());
    let jb = load("JetBrainsMono-Regular.ttf", "jbmono", plain);
    drop(load);

    // Phosphor is vertically centred a little low next to Inter.
    let icon_tweak = FontTweak { y_offset_factor: 0.08, ..Default::default() };
    fonts.font_data.insert(
        "phosphor".into(),
        FontData::from_static(egui_phosphor::Variant::Regular.font_bytes())
            .tweak(icon_tweak)
            .into(),
    );

    let default_prop = fonts.families[&FontFamily::Proportional].clone();
    let default_mono = fonts.families[&FontFamily::Monospace].clone();
    let family = |first: Option<&str>, rest: &[String]| {
        let mut v: Vec<String> = first.map(str::to_owned).into_iter().collect();
        v.push("phosphor".into());
        v.extend(rest.iter().cloned());
        v
    };
    let prop_first = inter.then_some("inter");
    fonts
        .families
        .insert(FontFamily::Proportional, family(prop_first, &default_prop));
    fonts
        .families
        .insert(FontFamily::Monospace, family(jb.then_some("jbmono"), &default_mono));
    fonts.families.insert(
        FontFamily::Name("medium".into()),
        family(inter_medium.then_some("inter-medium").or(prop_first), &default_prop),
    );
    fonts.families.insert(
        FontFamily::Name("semibold".into()),
        family(inter_semibold.then_some("inter-semibold").or(prop_first), &default_prop),
    );
    fonts
        .families
        .insert(FontFamily::Name("icons".into()), family(None, &default_prop));
    ctx.set_fonts(fonts);
    if found.is_empty() {
        "bundled egui fonts (S4_FONT_DIR empty)".into()
    } else {
        found.join(", ")
    }
}

fn visuals(p: &Palette) -> Visuals {
    let mut v = if p.dark { Visuals::dark() } else { Visuals::light() };
    let r = CornerRadius::same(size::RADIUS_SMALL);
    v.override_text_color = Some(p.text);
    v.weak_text_color = Some(p.text_weak);
    v.panel_fill = p.bg;
    v.window_fill = p.surface;
    v.window_stroke = Stroke::new(1.0, p.border);
    v.window_corner_radius = CornerRadius::same(size::RADIUS);
    v.menu_corner_radius = CornerRadius::same(size::RADIUS);
    v.window_shadow = Shadow {
        offset: [0, 4],
        blur: 16,
        spread: 0,
        color: Color32::from_black_alpha(if p.dark { 110 } else { 40 }),
    };
    v.popup_shadow = Shadow {
        offset: [0, 3],
        blur: 12,
        spread: 0,
        color: Color32::from_black_alpha(if p.dark { 110 } else { 36 }),
    };
    v.extreme_bg_color = p.surface;
    v.faint_bg_color = p.surface_alt;
    v.code_bg_color = p.surface_alt;
    v.hyperlink_color = p.accent;
    v.selection.bg_fill = if p.dark {
        Color32::from_rgb(0x2A, 0x45, 0x80)
    } else {
        Color32::from_rgb(0xC8, 0xD8, 0xFF)
    };
    v.selection.stroke = Stroke::new(1.0, p.accent);
    v.indent_has_left_vline = false;

    let w = &mut v.widgets;
    w.noninteractive.bg_stroke = Stroke::new(1.0, p.border);
    w.noninteractive.fg_stroke = Stroke::new(1.0, p.text);
    w.noninteractive.corner_radius = r;
    for (wv, fill, stroke) in [
        (&mut w.inactive, p.surface, p.border),
        (&mut w.hovered, p.hover, p.border),
        (&mut w.active, p.hover, p.accent),
        (&mut w.open, p.hover, p.border),
    ] {
        wv.bg_fill = fill;
        wv.weak_bg_fill = fill;
        wv.bg_stroke = Stroke::new(1.0, stroke);
        wv.corner_radius = r;
        wv.fg_stroke = Stroke::new(1.0, p.text);
        wv.expansion = 0.0;
    }
    v
}

pub fn install_styles(ctx: &egui::Context) {
    for (theme, p) in [(Theme::Dark, &DARK), (Theme::Light, &LIGHT)] {
        ctx.set_visuals_of(theme, visuals(p));
        ctx.style_mut_of(theme, |s| {
            s.text_styles = [
                (TextStyle::Small, body(size::SMALL)),
                (TextStyle::Body, body(size::BODY)),
                (TextStyle::Button, body(size::BODY)),
                (TextStyle::Heading, semibold(size::HEADING)),
                (TextStyle::Monospace, mono(size::MONO)),
            ]
            .into();
            s.spacing.item_spacing = egui::vec2(size::GAP, 4.0);
            s.spacing.button_padding = egui::vec2(8.0, 4.0);
            s.spacing.interact_size.y = 26.0;
            s.spacing.window_margin = Margin::same(size::PAD);
            s.spacing.menu_margin = Margin::same(6);
            s.interaction.tooltip_delay = 0.35;
        });
    }
}
