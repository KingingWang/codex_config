//! The look of the app: colours, fonts (with CJK support) and widget defaults.

use eframe::egui::{self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, Margin, Stroke, Style, TextStyle, Visuals};

pub const ACCENT: Color32 = Color32::from_rgb(0x5B, 0x8C, 0xFF);
pub const ACCENT_HI: Color32 = Color32::from_rgb(0x7E, 0xA6, 0xFF);
pub const ACCENT_WEAK: Color32 = Color32::from_rgb(0x1B, 0x2A, 0x4A);
pub const ACCENT_TEXT: Color32 = Color32::from_rgb(0xC7, 0xDB, 0xFF);

pub const BG: Color32 = Color32::from_rgb(0x0B, 0x0E, 0x14);
pub const PANEL: Color32 = Color32::from_rgb(0x10, 0x14, 0x1C);
pub const CARD: Color32 = Color32::from_rgb(0x16, 0x1B, 0x25);
pub const CARD_ALT: Color32 = Color32::from_rgb(0x1D, 0x24, 0x30);
pub const INPUT_BG: Color32 = Color32::from_rgb(0x0E, 0x12, 0x19);
pub const BORDER: Color32 = Color32::from_rgb(0x25, 0x2E, 0x3C);
pub const BORDER_STRONG: Color32 = Color32::from_rgb(0x37, 0x44, 0x56);

pub const TEXT: Color32 = Color32::from_rgb(0xEC, 0xF1, 0xF8);
pub const TEXT_DIM: Color32 = Color32::from_rgb(0xA2, 0xB0, 0xC1);
pub const TEXT_MUTED: Color32 = Color32::from_rgb(0x6E, 0x7B, 0x8D);

pub const OK: Color32 = Color32::from_rgb(0x53, 0xE0, 0x8C);
pub const OK_WEAK: Color32 = Color32::from_rgb(0x11, 0x2C, 0x1F);
pub const WARN: Color32 = Color32::from_rgb(0xF5, 0xC5, 0x59);
pub const WARN_WEAK: Color32 = Color32::from_rgb(0x2E, 0x26, 0x12);
pub const DANGER: Color32 = Color32::from_rgb(0xFF, 0x74, 0x74);
pub const DANGER_WEAK: Color32 = Color32::from_rgb(0x33, 0x18, 0x1B);

/// Fonts we try, in order, for CJK glyph coverage.
const CJK_CANDIDATES: &[&str] = &[
    "/System/Library/Fonts/PingFang.ttc",
    "/System/Library/PrivateFrameworks/FontServices.framework/Versions/A/Resources/Fonts/PingFang.ttc",
    "/System/Library/Fonts/Hiragino Sans GB.ttc",
    "/System/Library/Fonts/STHeiti Light.ttc",
    "/System/Library/Fonts/Supplemental/Arial Unicode.ttf",
    "/System/Library/Fonts/Supplemental/Songti.ttc",
    "C:/Windows/Fonts/msyh.ttc",
    "C:/Windows/Fonts/simhei.ttf",
    "C:/Windows/Fonts/msyhbd.ttc",
    "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
    "/usr/share/fonts/opentype/noto/NotoSerifCJK-Regular.ttc",
    "/usr/share/fonts/truetype/wqy/wqy-zenhei.ttc",
    "/usr/share/fonts/truetype/wqy/wqy-microhei.ttc",
    "/usr/share/fonts/truetype/arphic/uming.ttc",
    "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
];

const MONO_CANDIDATES: &[&str] = &[
    "/System/Library/Fonts/Menlo.ttc",
    "/System/Library/Fonts/Monaco.ttf",
    "/System/Library/Fonts/SFMono.ttf",
    "C:/Windows/Fonts/consola.ttf",
    "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
];

/// Name of the font that ended up providing CJK glyphs (shown in the About box).
pub fn install(ctx: &egui::Context) -> Option<String> {
    let mut fonts = FontDefinitions::default();
    let mut loaded: Option<String> = None;

    if let Some((name, bytes)) = first_readable(CJK_CANDIDATES) {
        fonts
            .font_data
            .insert(name.clone(), std::sync::Arc::new(FontData::from_owned(bytes)));
        fonts
            .families
            .entry(FontFamily::Proportional)
            .or_default()
            .push(name.clone());
        fonts
            .families
            .entry(FontFamily::Monospace)
            .or_default()
            .push(name.clone());
        loaded = Some(name);
    }

    if let Some((name, bytes)) = first_readable(MONO_CANDIDATES) {
        let key = format!("{name}-mono");
        fonts
            .font_data
            .insert(key.clone(), std::sync::Arc::new(FontData::from_owned(bytes)));
        // Put the mono face first in the monospace family, keep CJK as fallback.
        let family = fonts.families.entry(FontFamily::Monospace).or_default();
        family.insert(0, key);
    }

    fonts
        .families
        .entry(FontFamily::Proportional)
        .or_default()
        .sort_by_key(|name| if name.contains("emoji") || name.contains("Emoji") { 1 } else { 0 });

    // Phosphor gives us a full set of crisp vector icons that mix straight into
    // ordinary labels — no more relying on lone CJK glyphs as makeshift icons.
    egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);

    ctx.set_fonts(fonts);
    ctx.options_mut(|options| options.theme_preference = egui::ThemePreference::Dark);
    ctx.set_style_of(egui::Theme::Dark, style());
    loaded
}

fn first_readable(candidates: &[&str]) -> Option<(String, Vec<u8>)> {
    for path in candidates {
        if let Ok(bytes) = std::fs::read(path)
            && !bytes.is_empty()
        {
            let name = std::path::Path::new(path)
                .file_stem()
                .map(|s| s.to_string_lossy().replace(' ', "_"))
                .unwrap_or_else(|| "system".to_string());
            return Some((name, bytes));
        }
    }
    None
}

pub fn style() -> Style {
    let mut style = Style::default();

    style.text_styles = [
        (TextStyle::Heading, egui::FontId::proportional(21.0)),
        (TextStyle::Body, egui::FontId::proportional(14.5)),
        (TextStyle::Monospace, egui::FontId::monospace(13.0)),
        (TextStyle::Button, egui::FontId::proportional(14.0)),
        (TextStyle::Small, egui::FontId::proportional(12.0)),
    ]
    .into();

    style.spacing.item_spacing = egui::vec2(8.0, 8.0);
    style.spacing.button_padding = egui::vec2(12.0, 7.0);
    style.spacing.interact_size = egui::vec2(120.0, 26.0);
    style.spacing.combo_width = 90.0;
    style.spacing.scroll = egui::style::ScrollStyle {
        bar_width: 9.0,
        ..Default::default()
    };
    style.spacing.window_margin = Margin::same(12);

    let mut visuals = Visuals::dark();
    visuals.panel_fill = BG;
    visuals.window_fill = PANEL;
    visuals.extreme_bg_color = INPUT_BG;
    visuals.faint_bg_color = CARD;
    visuals.dark_mode = true;
    visuals.hyperlink_color = ACCENT_TEXT;
    visuals.selection.bg_fill = ACCENT.linear_multiply(0.35);
    visuals.selection.stroke = Stroke::new(1.0, ACCENT_TEXT);
    visuals.window_corner_radius = CornerRadius::same(14);
    visuals.menu_corner_radius = CornerRadius::same(10);
    visuals.window_shadow = egui::epaint::Shadow {
        offset: [0, 6],
        spread: 24,
        blur: 40,
        color: Color32::from_black_alpha(140),
    };
    visuals.popup_shadow = visuals.window_shadow;

    // Non interactive widgets (labels, frames)
    visuals.widgets.noninteractive.bg_fill = CARD;
    visuals.widgets.noninteractive.weak_bg_fill = CARD;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, BORDER);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT);
    visuals.widgets.noninteractive.corner_radius = CornerRadius::same(10);
    visuals.widgets.noninteractive.expansion = 0.0;

    // Buttons, checkboxes, ...
    visuals.widgets.inactive.bg_fill = CARD_ALT;
    visuals.widgets.inactive.weak_bg_fill = CARD_ALT;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, BORDER);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT);
    visuals.widgets.inactive.corner_radius = CornerRadius::same(8);
    visuals.widgets.inactive.expansion = 0.0;

    visuals.widgets.hovered.bg_fill = Color32::from_rgb(0x24, 0x2D, 0x3A);
    visuals.widgets.hovered.weak_bg_fill = Color32::from_rgb(0x24, 0x2D, 0x3A);
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, BORDER_STRONG);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.25, Color32::WHITE);
    visuals.widgets.hovered.corner_radius = CornerRadius::same(8);
    visuals.widgets.hovered.expansion = 0.5;

    visuals.widgets.active.bg_fill = ACCENT_WEAK;
    visuals.widgets.active.weak_bg_fill = ACCENT_WEAK;
    visuals.widgets.active.bg_stroke = Stroke::new(1.0, ACCENT);
    visuals.widgets.active.fg_stroke = Stroke::new(1.25, ACCENT_TEXT);
    visuals.widgets.active.corner_radius = CornerRadius::same(8);
    visuals.widgets.active.expansion = 0.5;

    visuals.widgets.open.bg_fill = CARD_ALT;
    visuals.widgets.open.weak_bg_fill = CARD_ALT;
    visuals.widgets.open.bg_stroke = Stroke::new(1.0, BORDER_STRONG);
    visuals.widgets.open.fg_stroke = Stroke::new(1.0, TEXT);
    visuals.widgets.open.corner_radius = CornerRadius::same(8);

    style.visuals = visuals;
    style
}

/// Frame used for content "cards".
pub fn card_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(CARD)
        .stroke(Stroke::new(1.0, BORDER))
        .corner_radius(CornerRadius::same(14))
        .inner_margin(Margin::same(20))
}

pub fn subtle_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(INPUT_BG)
        .stroke(Stroke::new(1.0, BORDER))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(Margin::same(12))
}

/// A soft-tinted frame keyed to an accent colour, used behind section icons and
/// highlight callouts.
pub fn tint_frame(color: Color32, radius: u8) -> egui::Frame {
    egui::Frame::new()
        .fill(color.gamma_multiply(0.16))
        .stroke(Stroke::new(1.0, color.gamma_multiply(0.42)))
        .corner_radius(CornerRadius::same(radius))
}
