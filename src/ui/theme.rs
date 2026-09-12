//! Charcoal workbench: neutral dark surfaces and restrained blue accents.
//! Shared tokens for every page, dialog and editor (see DESIGN.md).

use eframe::egui::{
    self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, Margin, Stroke, Style,
    TextStyle, Visuals,
};

pub const BG: Color32 = Color32::from_rgb(0x11, 0x13, 0x18);
pub const PANEL: Color32 = Color32::from_rgb(0x15, 0x18, 0x1E);
pub const CARD: Color32 = Color32::from_rgb(0x1C, 0x20, 0x28);
pub const CARD_ALT: Color32 = Color32::from_rgb(0x23, 0x28, 0x32);
pub const INPUT_BG: Color32 = Color32::from_rgb(0x12, 0x15, 0x1B);
pub const BORDER: Color32 = Color32::from_rgb(0x30, 0x37, 0x44);
pub const BORDER_STRONG: Color32 = Color32::from_rgb(0x49, 0x55, 0x69);
pub const TEXT: Color32 = Color32::from_rgb(0xE7, 0xEB, 0xF2);
pub const TEXT_DIM: Color32 = Color32::from_rgb(0xB5, 0xBE, 0xCC);
pub const TEXT_MUTED: Color32 = Color32::from_rgb(0x9B, 0xA6, 0xB8);
pub const ACCENT: Color32 = Color32::from_rgb(0x71, 0x9D, 0xF4);
pub const ACCENT_HI: Color32 = Color32::from_rgb(0xA4, 0xC2, 0xFF);
pub const ACCENT_WEAK: Color32 = Color32::from_rgb(0x24, 0x32, 0x4B);
pub const ACCENT_TEXT: Color32 = Color32::from_rgb(0xBD, 0xD2, 0xFF);
pub const SELECTION: Color32 = Color32::from_rgb(0x38, 0x4D, 0x70);
pub const SELECTION_WEAK: Color32 = Color32::from_rgb(0x29, 0x34, 0x47);
pub const OK: Color32 = Color32::from_rgb(0x83, 0xC4, 0xA3);
pub const OK_WEAK: Color32 = Color32::from_rgb(0x20, 0x32, 0x2E);
pub const WARN: Color32 = Color32::from_rgb(0xEF, 0xBF, 0x72);
pub const WARN_WEAK: Color32 = Color32::from_rgb(0x38, 0x2E, 0x22);
pub const DANGER: Color32 = Color32::from_rgb(0xF0, 0x92, 0x98);
pub const DANGER_WEAK: Color32 = Color32::from_rgb(0x3C, 0x27, 0x2E);
pub const INFO: Color32 = ACCENT;
pub const INFO_WEAK: Color32 = ACCENT_WEAK;

// -- Fonts ------------------------------------------------------------------

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
        fonts.font_data.insert(
            name.clone(),
            std::sync::Arc::new(FontData::from_owned(bytes)),
        );
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
        fonts.font_data.insert(
            key.clone(),
            std::sync::Arc::new(FontData::from_owned(bytes)),
        );
        // Put the mono face first in the monospace family, keep CJK as fallback.
        let family = fonts.families.entry(FontFamily::Monospace).or_default();
        family.insert(0, key);
    }

    fonts
        .families
        .entry(FontFamily::Proportional)
        .or_default()
        .sort_by_key(|name| {
            if name.contains("emoji") || name.contains("Emoji") {
                1
            } else {
                0
            }
        });

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
    let mut visuals = Visuals::dark();
    visuals.panel_fill = BG;
    visuals.window_fill = CARD;
    visuals.extreme_bg_color = INPUT_BG;
    visuals.faint_bg_color = CARD_ALT;
    visuals.dark_mode = true;
    visuals.hyperlink_color = ACCENT_HI;
    visuals.selection.bg_fill = SELECTION;
    visuals.selection.stroke = Stroke::new(1.0, ACCENT);
    visuals.window_corner_radius = CornerRadius::same(16);
    visuals.menu_corner_radius = CornerRadius::same(12);
    // Subtle shadows for depth, not heavy
    visuals.window_shadow = egui::epaint::Shadow {
        offset: [0, 4],
        spread: 12,
        blur: 24,
        color: Color32::from_black_alpha(25),
    };
    visuals.popup_shadow = egui::epaint::Shadow {
        offset: [0, 2],
        spread: 8,
        blur: 16,
        color: Color32::from_black_alpha(20),
    };

    // Non interactive widgets (labels, frames)
    visuals.widgets.noninteractive.bg_fill = CARD;
    visuals.widgets.noninteractive.weak_bg_fill = CARD_ALT;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, BORDER);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT);
    visuals.widgets.noninteractive.corner_radius = CornerRadius::same(12);
    visuals.widgets.noninteractive.expansion = 0.0;

    // Buttons, checkboxes, ...
    visuals.widgets.inactive.bg_fill = CARD_ALT;
    visuals.widgets.inactive.weak_bg_fill = CARD_ALT;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, BORDER);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT);
    visuals.widgets.inactive.corner_radius = CornerRadius::same(10);
    visuals.widgets.inactive.expansion = 0.0;

    visuals.widgets.hovered.bg_fill = SELECTION_WEAK;
    visuals.widgets.hovered.weak_bg_fill = SELECTION_WEAK;
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, BORDER_STRONG);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.25, TEXT);
    visuals.widgets.hovered.corner_radius = CornerRadius::same(10);
    visuals.widgets.hovered.expansion = 0.5;

    visuals.widgets.active.bg_fill = ACCENT_WEAK;
    visuals.widgets.active.weak_bg_fill = ACCENT_WEAK;
    visuals.widgets.active.bg_stroke = Stroke::new(1.0, ACCENT);
    visuals.widgets.active.fg_stroke = Stroke::new(1.25, ACCENT_TEXT);
    visuals.widgets.active.corner_radius = CornerRadius::same(10);
    visuals.widgets.active.expansion = 0.5;

    visuals.widgets.open.bg_fill = CARD_ALT;
    visuals.widgets.open.weak_bg_fill = CARD_ALT;
    visuals.widgets.open.bg_stroke = Stroke::new(1.0, BORDER_STRONG);
    visuals.widgets.open.fg_stroke = Stroke::new(1.0, TEXT);
    visuals.widgets.open.corner_radius = CornerRadius::same(10);

    Style {
        text_styles: [
            (TextStyle::Heading, egui::FontId::proportional(26.0)), // Large headings
            (TextStyle::Body, egui::FontId::proportional(15.0)),    // Body text
            (TextStyle::Monospace, egui::FontId::monospace(13.5)),  // Code/IDs
            (TextStyle::Button, egui::FontId::proportional(14.5)),  // Button text
            (TextStyle::Small, egui::FontId::proportional(13.0)),   // Small text
        ]
        .into(),
        spacing: egui::Spacing {
            item_spacing: egui::vec2(10.0, 10.0),
            button_padding: egui::vec2(12.0, 8.0),
            interact_size: egui::vec2(120.0, 36.0),
            combo_width: 100.0,
            scroll: egui::style::ScrollStyle {
                bar_width: 10.0,
                ..Default::default()
            },
            window_margin: Margin::same(16),
            ..Default::default()
        },
        visuals,
        ..Default::default()
    }
}

/// Raised dark content surface with a subtle border.
pub fn card_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(CARD)
        .stroke(Stroke::new(1.0, BORDER))
        .corner_radius(CornerRadius::same(16))
        .inner_margin(Margin::same(20))
}

/// Subtle frame for input areas and secondary content.
pub fn subtle_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(INPUT_BG)
        .stroke(Stroke::new(1.0, BORDER))
        .corner_radius(CornerRadius::same(12))
        .inner_margin(Margin::same(14))
}

/// A soft-tinted frame keyed to an accent colour, used behind section icons and
/// highlight callouts.
pub fn tint_frame(color: Color32, radius: u8) -> egui::Frame {
    egui::Frame::new()
        .fill(color.gamma_multiply(0.12))
        .stroke(Stroke::new(1.0, color.gamma_multiply(0.3)))
        .corner_radius(CornerRadius::same(radius))
}

/// Panel background frame - slightly darker than main canvas.
pub fn panel_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(PANEL)
        .stroke(Stroke::new(1.0, BORDER))
        .inner_margin(Margin::same(16))
}
