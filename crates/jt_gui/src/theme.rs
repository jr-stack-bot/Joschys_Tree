// Joschys_Tree – Design "Reaktor": Palette dunkel/hell, egui-Stil, Schriften, Treemap-Farben – @ Joschy
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::sync::Arc;

use eframe::egui::{
    self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Id, Margin, Shadow,
    Stroke, TextStyle, Visuals,
};

/// Zentrale Farbpalette. Alle Ansichten nehmen ihre Farben nur von hier.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Theme {
    pub dark: bool,
    /// Fensterhintergrund.
    pub bg: Color32,
    /// Panel/Karte (innere Fläche).
    pub panel: Color32,
    /// Zeilen-/Knopfgrund, Zebrastreifen.
    pub row: Color32,
    /// Hover-/Auswahlzeile.
    pub row_hover: Color32,
    /// Rand und Trennlinien.
    pub line: Color32,
    /// Kopf-Verlauf oben/unten (Kopfbereich, Detailpanel).
    pub core_hi: Color32,
    pub core_lo: Color32,
    pub text: Color32,
    pub text_hi: Color32,
    pub text2: Color32,
    pub dim: Color32,
    /// Akzent für Linien, Leisten und Flächen.
    pub accent: Color32,
    /// Helles Verlaufsende.
    pub accent_hi: Color32,
    /// Akzent für Schrift (im Hell-Modus dunkler, wegen Kontrast).
    pub accent_text: Color32,
    /// Fläche aktiver Tabs/Primärknopf und Text darauf.
    pub accent_fill: Color32,
    pub on_accent: Color32,
    /// Text der Auswahlzeile.
    pub sel_text: Color32,
    pub red: Color32,
    pub warn: Color32,
    pub warn_hi: Color32,
    pub crit: Color32,
    pub crit_hi: Color32,
    pub ok: Color32,
    pub err: Color32,
    /// Leucht-Effekte (nur dunkel sinnvoll).
    pub glow: bool,
}

const fn rgb(hex: u32) -> Color32 {
    Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

/// Original "Reaktor"-Palette des Kreis-Konverters.
pub const DARK: Theme = Theme {
    dark: true,
    bg: rgb(0x060b0e),
    panel: rgb(0x0a1317),
    row: rgb(0x0c2124),
    row_hover: rgb(0x114141),
    line: rgb(0x1d3a40),
    core_hi: rgb(0x12262d),
    core_lo: rgb(0x03070a),
    text: rgb(0xdff7f4),
    text_hi: rgb(0xffffff),
    text2: rgb(0x86a9ad),
    dim: rgb(0x6baaaa),
    accent: rgb(0x2ee6d6),
    accent_hi: rgb(0xa0f3ec),
    accent_text: rgb(0x2ee6d6),
    accent_fill: rgb(0x2ee6d6),
    on_accent: rgb(0x060b0e),
    sel_text: rgb(0x2ee6d6),
    red: rgb(0xff3b30),
    warn: rgb(0xffb000),
    warn_hi: rgb(0xff8a00),
    crit: rgb(0xff3b30),
    crit_hi: rgb(0xff7d13),
    ok: rgb(0x00ff9d),
    err: rgb(0xff5c7a),
    glow: true,
};

/// Abgeleitete helle Variante (im Original nicht vorhanden).
pub const LIGHT: Theme = Theme {
    dark: false,
    bg: rgb(0xeef5f5),
    panel: rgb(0xf8fcfc),
    row: rgb(0xe3eeee),
    row_hover: rgb(0xc4e6e3),
    line: rgb(0xb4cfd1),
    core_hi: rgb(0xf8fcfc),
    core_lo: rgb(0xe3eeee),
    text: rgb(0x0b1f23),
    text_hi: rgb(0x0b1f23),
    text2: rgb(0x46666b),
    dim: rgb(0x557276),
    accent: rgb(0x0a9c91),
    accent_hi: rgb(0x3fd9cc),
    accent_text: rgb(0x077a72),
    accent_fill: rgb(0x077a72),
    on_accent: rgb(0xffffff),
    sel_text: rgb(0x07524c),
    red: rgb(0xd6291f),
    warn: rgb(0xb07500),
    warn_hi: rgb(0xe0a020),
    crit: rgb(0xd6291f),
    crit_hi: rgb(0xe8642a),
    ok: rgb(0x087a4d),
    err: rgb(0xc42a45),
    glow: false,
};

impl Theme {
    pub fn get(dark: bool) -> &'static Theme {
        if dark { &DARK } else { &LIGHT }
    }

    /// Palette passend zum aktuellen Modus der Oberfläche.
    pub fn of(ui: &egui::Ui) -> &'static Theme {
        Self::get(ui.visuals().dark_mode)
    }
}

// ---------------------------------------------------------------------------
// Farbrechnung (rein, getestet)

/// Lineare Mischung zweier Farben in sRGB, `t` = 0 ergibt `a`, 1 ergibt `b`.
pub fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let ch = |x: u8, y: u8| (f32::from(x) + (f32::from(y) - f32::from(x)) * t).round() as u8;
    Color32::from_rgba_unmultiplied(
        ch(a.r(), b.r()),
        ch(a.g(), b.g()),
        ch(a.b(), b.b()),
        ch(a.a(), b.a()),
    )
}

/// Farbe mit Deckkraft 0..=255 (nicht vormultipliziert angegeben).
pub fn with_alpha(c: Color32, alpha: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), alpha)
}

/// Relative Leuchtdichte nach WCAG 2.x.
pub fn luminance(c: Color32) -> f64 {
    let lin = |v: u8| {
        let s = f64::from(v) / 255.0;
        if s <= 0.039_28 {
            s / 12.92
        } else {
            ((s + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * lin(c.r()) + 0.7152 * lin(c.g()) + 0.0722 * lin(c.b())
}

/// Kontrastverhältnis nach WCAG (1..21).
pub fn contrast(a: Color32, b: Color32) -> f64 {
    let (la, lb) = (luminance(a), luminance(b));
    let (hi, lo) = if la > lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

/// Gut lesbare Schriftfarbe (hell oder dunkel) auf einer Fläche.
pub fn readable_on(bg: Color32) -> Color32 {
    let light = DARK.text;
    let dark = rgb(0x060b0e);
    if contrast(light, bg) >= contrast(dark, bg) {
        light
    } else {
        dark
    }
}

/// Stufe eines Anteilsbalkens.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BarLevel {
    Normal,
    Warn,
    Critical,
}

/// Ab diesem Anteil wird der Balken bernsteinfarben bzw. rot.
pub const WARN_FRAC: f32 = 0.6;
pub const CRIT_FRAC: f32 = 0.9;

pub fn bar_level(frac: f32) -> BarLevel {
    if frac >= CRIT_FRAC {
        BarLevel::Critical
    } else if frac >= WARN_FRAC {
        BarLevel::Warn
    } else {
        BarLevel::Normal
    }
}

/// Verlauf (Start, Ende) eines Anteilsbalkens.
pub fn bar_gradient(t: &Theme, level: BarLevel) -> (Color32, Color32) {
    match level {
        BarLevel::Normal => (t.accent, t.accent_hi),
        BarLevel::Warn => (t.warn, t.warn_hi),
        BarLevel::Critical => (t.crit, t.crit_hi),
    }
}

// ---------------------------------------------------------------------------
// Dateitypen-Kategorien für Treemap und Dateitypen-Liste

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileCat {
    Image,
    Office,
    Pdf,
    Data,
    Text,
    Archive,
    Audio,
    Video,
    Program,
    Other,
}

impl FileCat {
    /// Einordnung nach Dateiendung (ohne Punkt, Groß-/Kleinschreibung egal).
    pub fn from_ext(ext: &str) -> FileCat {
        let mut buf = [0u8; 8];
        let b = ext.as_bytes();
        if b.is_empty() || b.len() > buf.len() {
            return FileCat::Other;
        }
        for (d, s) in buf.iter_mut().zip(b) {
            *d = s.to_ascii_lowercase();
        }
        let Ok(e) = std::str::from_utf8(&buf[..b.len()]) else {
            return FileCat::Other;
        };
        match e {
            "jpg" | "jpeg" | "png" | "gif" | "bmp" | "tif" | "tiff" | "heic" | "heif" | "webp"
            | "svg" | "ico" | "psd" | "raw" | "cr2" | "cr3" | "nef" | "arw" | "dng" => {
                FileCat::Image
            }
            "doc" | "docx" | "docm" | "odt" | "rtf" | "ppt" | "pptx" | "odp" | "one" | "msg"
            | "pst" | "ost" | "vsd" | "vsdx" | "pub" => FileCat::Office,
            "pdf" | "xps" => FileCat::Pdf,
            "xls" | "xlsx" | "xlsm" | "ods" | "csv" | "db" | "sqlite" | "mdb" | "accdb" | "mdf"
            | "ldf" | "json" | "xml" | "sql" | "bak" | "edb" | "dat" => FileCat::Data,
            "txt" | "md" | "log" | "ini" | "cfg" | "conf" | "epub" | "mobi" | "azw3" | "htm"
            | "html" | "rs" | "py" | "js" | "ts" | "c" | "h" | "cpp" | "cs" | "java" | "ps1"
            | "bat" | "cmd" | "sh" | "yml" | "yaml" | "toml" => FileCat::Text,
            "zip" | "7z" | "rar" | "tar" | "gz" | "tgz" | "bz2" | "xz" | "zst" | "cab" | "iso"
            | "img" | "vhd" | "vhdx" | "vmdk" | "wim" | "esd" | "deb" | "rpm" | "jar" => {
                FileCat::Archive
            }
            "mp3" | "wav" | "flac" | "aac" | "ogg" | "m4a" | "wma" | "opus" | "aiff" | "mid" => {
                FileCat::Audio
            }
            "mp4" | "mkv" | "avi" | "mov" | "wmv" | "webm" | "m4v" | "mpg" | "mpeg" | "ts2"
            | "m2ts" | "flv" | "3gp" | "vob" => FileCat::Video,
            "exe" | "dll" | "sys" | "msi" | "msp" | "so" | "a" | "o" | "lib" | "bin" | "drv"
            | "ocx" | "cpl" | "mui" | "efi" | "pdb" | "node" | "pyc" | "class" | "wasm" => {
                FileCat::Program
            }
            _ => FileCat::Other,
        }
    }

    /// Volle Kategoriefarbe (Kontur, Kennquadrat).
    pub fn color(self) -> Color32 {
        match self {
            FileCat::Image => rgb(0x00f0ff),
            FileCat::Office => rgb(0x2b579a),
            FileCat::Pdf => rgb(0xe74c3c),
            FileCat::Data => rgb(0xff9900),
            FileCat::Text => rgb(0x9b59b6),
            FileCat::Archive => rgb(0x2ecc71),
            FileCat::Audio => rgb(0xff2bd6),
            FileCat::Video => rgb(0xffb000),
            FileCat::Program => rgb(0x8a5cff),
            FileCat::Other => rgb(0x86a9ad),
        }
    }

    /// Kachelfüllung in der Treemap (dunkel: 62 % Farbe + 38 % Panel).
    pub fn tile(self, t: &Theme) -> Color32 {
        if t.dark {
            lerp_color(t.panel, self.color(), 0.62)
        } else {
            match self {
                FileCat::Image => rgb(0x00a9b8),
                FileCat::Office => rgb(0x2b579a),
                FileCat::Pdf => rgb(0xd64233),
                FileCat::Data => rgb(0xe08600),
                FileCat::Text => rgb(0x8a4ea6),
                FileCat::Archive => rgb(0x24a85e),
                FileCat::Audio => rgb(0xd41fb2),
                FileCat::Video => rgb(0xd99200),
                FileCat::Program => rgb(0x7549e0),
                FileCat::Other => rgb(0x8fb0b4),
            }
        }
    }
}

// ---------------------------------------------------------------------------
// egui-Stil

fn widget(
    bg: Color32,
    weak: Color32,
    stroke: Stroke,
    fg: Color32,
    fg_w: f32,
) -> egui::style::WidgetVisuals {
    egui::style::WidgetVisuals {
        bg_fill: bg,
        weak_bg_fill: weak,
        bg_stroke: stroke,
        corner_radius: CornerRadius::ZERO,
        fg_stroke: Stroke::new(fg_w, fg),
        expansion: 0.0,
    }
}

/// egui-Visuals vollständig aus der Palette.
pub fn visuals(t: &Theme) -> Visuals {
    let mut v = if t.dark {
        Visuals::dark()
    } else {
        Visuals::light()
    };
    let line = Stroke::new(1.0, t.line);
    v.override_text_color = None;
    v.weak_text_color = Some(t.text2);
    v.panel_fill = t.bg;
    v.window_fill = t.panel;
    v.window_stroke = line;
    v.window_corner_radius = CornerRadius::ZERO;
    v.menu_corner_radius = CornerRadius::ZERO;
    let glow = Shadow {
        offset: [0, 0],
        blur: 14,
        spread: 0,
        color: with_alpha(t.accent, 40),
    };
    v.window_shadow = if t.glow { glow } else { Shadow::NONE };
    v.popup_shadow = if t.glow {
        Shadow { blur: 8, ..glow }
    } else {
        Shadow::NONE
    };
    v.extreme_bg_color = t.bg;
    v.text_edit_bg_color = Some(t.bg);
    v.faint_bg_color = t.row;
    v.code_bg_color = t.row;
    v.hyperlink_color = t.accent_text;
    v.warn_fg_color = t.warn;
    v.error_fg_color = t.err;
    v.selection.bg_fill = t.row_hover;
    v.selection.stroke = Stroke::new(1.0, t.sel_text);
    v.striped = true;
    v.indent_has_left_vline = true;
    v.slider_trailing_fill = true;
    v.handle_shape = egui::style::HandleShape::Rect { aspect_ratio: 0.5 };
    v.text_cursor.stroke = Stroke::new(2.0, t.accent);
    v.button_frame = true;
    v.widgets.noninteractive = widget(t.panel, t.panel, line, t.text, 1.0);
    v.widgets.inactive = widget(t.row, t.row, line, t.text, 1.0);
    v.widgets.hovered = widget(
        t.row_hover,
        t.row_hover,
        Stroke::new(1.0, t.accent),
        t.accent_text,
        1.5,
    );
    v.widgets.active = widget(
        t.accent_fill,
        t.accent_fill,
        Stroke::new(1.0, t.accent),
        t.on_accent,
        1.5,
    );
    v.widgets.open = widget(
        t.row_hover,
        t.row_hover,
        Stroke::new(1.0, t.accent),
        t.accent_text,
        1.0,
    );
    v
}

/// Abstände, Schriftgrößen und Bildlaufleisten (für beide Modi gleich).
pub fn tune_style(style: &mut egui::Style) {
    use FontFamily::{Monospace, Proportional};
    style.text_styles = [
        (TextStyle::Heading, FontId::new(19.0, Proportional)),
        (TextStyle::Body, FontId::new(13.0, Monospace)),
        (TextStyle::Monospace, FontId::new(13.0, Monospace)),
        (TextStyle::Button, FontId::new(14.0, Proportional)),
        (TextStyle::Small, FontId::new(11.0, Monospace)),
    ]
    .into();
    let s = &mut style.spacing;
    s.item_spacing = egui::vec2(6.0, 4.0);
    s.button_padding = egui::vec2(9.0, 3.0);
    s.window_margin = Margin::same(12);
    s.menu_margin = Margin::same(6);
    s.interact_size.y = 20.0;
    s.scroll = egui::style::ScrollStyle::solid();
    s.scroll.bar_width = 5.0;
    s.scroll.bar_inner_margin = 2.0;
    s.scroll.bar_outer_margin = 0.0;
}

/// Theme setzen (Standard dunkel). Beide Varianten werden registriert.
pub fn apply(ctx: &egui::Context, dark: bool) {
    ctx.set_visuals_of(egui::Theme::Dark, visuals(&DARK));
    ctx.set_visuals_of(egui::Theme::Light, visuals(&LIGHT));
    ctx.all_styles_mut(tune_style);
    ctx.set_theme(if dark {
        egui::Theme::Dark
    } else {
        egui::Theme::Light
    });
}

fn effects_id() -> Id {
    Id::unique("jt_effects")
}

/// Zier-Effekte (Leuchten, Scanlines) ein-/ausschalten.
pub fn set_effects(ctx: &egui::Context, on: bool) {
    ctx.data_mut(|d| d.insert_temp(effects_id(), on));
}

/// Sind Zier-Effekte aktiv (und sinnvoll im aktuellen Modus)?
pub fn effects(ui: &egui::Ui) -> bool {
    ui.ctx()
        .data(|d| d.get_temp::<bool>(effects_id()))
        .unwrap_or(true)
}

// ---------------------------------------------------------------------------
// Schriften: Consolas/Bahnschrift zur Laufzeit aus C:\Windows\Fonts, nie eingebettet

/// Name der Schrift für Titel/Knöpfe (Proportional) und Listen (Monospace).
pub const FONT_TITLE: &str = "jt_title";
pub const FONT_MONO: &str = "jt_mono";

/// Welche Datei für welchen Zweck geladen wurde (für Tests/Diagnose).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FontChoice {
    pub title: Option<&'static str>,
    pub mono: Option<&'static str>,
}

/// Prüft, ob `bytes` eine Schrift ist, die epaint fehlerfrei laden kann.
///
/// epaint 0.37 parst Schriften in `FontFace::new` mit
/// `skrifa::FontRef::from_index(bytes, index)` und bricht bei einem Fehler in
/// `family.rs` mit `panic!` ab. Hier wird genau dieselbe Funktion (Index 0, wie bei
/// `FontData::from_owned`) aufgerufen und zusätzlich strenger geprüft: alle Tabellen
/// liegen innerhalb der Datei, die Pflichttabellen sind lesbar und die Schrift hat
/// Zeichen und Umrisse. So fällt auch eine abgeschnittene Datei durch.
pub fn font_bytes_valid(bytes: &[u8]) -> bool {
    use skrifa::MetadataProvider as _;
    use skrifa::raw::TableProvider as _;
    let Ok(font) = skrifa::FontRef::from_index(bytes, 0) else {
        return false;
    };
    let records = font.table_directory.table_records();
    if records.is_empty() || !records.iter().all(|r| font.table_data(r.tag()).is_some()) {
        return false;
    }
    let Ok(head) = font.head() else {
        return false;
    };
    if head.units_per_em() == 0 || font.maxp().is_err() || font.hhea().is_err() {
        return false;
    }
    if font.cmap().is_err() {
        return false;
    }
    let Some(gid) = font.charmap().map('A') else {
        return false;
    };
    let glyphs = font.outline_glyphs();
    glyphs.format().is_some() && glyphs.get(gid).is_some()
}

/// Baut die Schrift-Definitionen. `read` liefert den Dateiinhalt einer Schrift
/// (Dateiname wie "consola.ttf") oder `None`. Nur gültige Dateien
/// ([`font_bytes_valid`]) werden registriert, sonst der nächste Kandidat;
/// fehlt alles, bleibt egui-Standard.
pub fn font_definitions(read: &dyn Fn(&str) -> Option<Vec<u8>>) -> (FontDefinitions, FontChoice) {
    let mut defs = FontDefinitions::default();
    let mut choice = FontChoice::default();
    let mut install = |name: &str, family: FontFamily, files: &[&'static str]| {
        for &f in files {
            if let Some(bytes) = read(f).filter(|b| font_bytes_valid(b)) {
                defs.font_data
                    .insert(name.to_owned(), Arc::new(FontData::from_owned(bytes)));
                defs.families
                    .entry(family)
                    .or_default()
                    .insert(0, name.to_owned());
                return Some(f);
            }
        }
        None
    };
    choice.title = install(
        FONT_TITLE,
        FontFamily::Proportional,
        &["bahnschrift.ttf", "segoeui.ttf"],
    );
    choice.mono = install(FONT_MONO, FontFamily::Monospace, &["consola.ttf"]);
    if choice.mono.is_none() {
        // Kein Consolas: Segoe UI (falls vorhanden) zumindest für Listen nutzen.
        choice.mono = install(FONT_MONO, FontFamily::Monospace, &["segoeui.ttf"]);
    }
    (defs, choice)
}

/// Lädt die Windows-Schriften; auf anderen Systemen bleibt der egui-Standard.
pub fn install_fonts(ctx: &egui::Context) {
    if !cfg!(windows) {
        return;
    }
    let dir = std::env::var_os("WINDIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("C:\\Windows"))
        .join("Fonts");
    let read = |f: &str| std::fs::read(dir.join(f)).ok();
    let (defs, _) = font_definitions(&read);
    ctx.set_fonts(defs);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dark_contrast_is_sufficient() {
        for bg in [DARK.bg, DARK.panel] {
            assert!(contrast(DARK.text, bg) >= 4.5);
            assert!(contrast(DARK.accent_text, bg) >= 4.5);
            assert!(contrast(DARK.text2, bg) >= 4.5);
        }
        assert!(contrast(DARK.sel_text, DARK.row_hover) >= 4.5);
        assert!(contrast(DARK.on_accent, DARK.accent_fill) >= 4.5);
    }

    #[test]
    fn light_contrast_is_sufficient() {
        for bg in [LIGHT.bg, LIGHT.panel] {
            assert!(contrast(LIGHT.text, bg) >= 7.0);
            assert!(contrast(LIGHT.accent_text, bg) >= 4.5);
            assert!(contrast(LIGHT.text2, bg) >= 4.5);
            assert!(contrast(LIGHT.warn, bg) >= 3.0);
        }
        assert!(contrast(LIGHT.sel_text, LIGHT.row_hover) >= 4.5);
        assert!(contrast(LIGHT.on_accent, LIGHT.accent_fill) >= 4.5);
        assert!(contrast(LIGHT.text, LIGHT.row_hover) >= 7.0);
    }

    #[test]
    fn contrast_reference_values() {
        assert!((contrast(Color32::BLACK, Color32::WHITE) - 21.0).abs() < 1e-6);
        assert!((contrast(DARK.bg, DARK.bg) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn gradient_endpoints_and_middle() {
        let a = rgb(0x000000);
        let b = rgb(0xff8040);
        assert_eq!(lerp_color(a, b, 0.0), a);
        assert_eq!(lerp_color(a, b, 1.0), b);
        assert_eq!(lerp_color(a, b, 0.5), rgb(0x804020));
        assert_eq!(lerp_color(a, b, -3.0), a);
        assert_eq!(lerp_color(a, b, 7.0), b);
        // Spezifikation: dunkle Kachel = 62 % Farbe + 38 % Panel
        assert_eq!(
            FileCat::Image.tile(&DARK),
            lerp_color(DARK.panel, rgb(0x00f0ff), 0.62)
        );
    }

    #[test]
    fn bar_levels_and_gradients() {
        assert_eq!(bar_level(0.1), BarLevel::Normal);
        assert_eq!(bar_level(WARN_FRAC), BarLevel::Warn);
        assert_eq!(bar_level(0.95), BarLevel::Critical);
        assert_eq!(
            bar_gradient(&DARK, BarLevel::Normal),
            (DARK.accent, DARK.accent_hi)
        );
        assert_eq!(bar_gradient(&LIGHT, BarLevel::Critical).0, LIGHT.crit);
    }

    #[test]
    fn readable_text_on_tiles() {
        for t in [&DARK, &LIGHT] {
            for c in [
                FileCat::Image,
                FileCat::Office,
                FileCat::Pdf,
                FileCat::Data,
                FileCat::Text,
                FileCat::Archive,
                FileCat::Audio,
                FileCat::Video,
                FileCat::Program,
                FileCat::Other,
            ] {
                let bg = c.tile(t);
                assert!(contrast(readable_on(bg), bg) >= 3.0, "{c:?}");
            }
        }
    }

    #[test]
    fn categories_by_extension() {
        assert_eq!(FileCat::from_ext("JPG"), FileCat::Image);
        assert_eq!(FileCat::from_ext("docx"), FileCat::Office);
        assert_eq!(FileCat::from_ext("pdf"), FileCat::Pdf);
        assert_eq!(FileCat::from_ext("xlsx"), FileCat::Data);
        assert_eq!(FileCat::from_ext("7z"), FileCat::Archive);
        assert_eq!(FileCat::from_ext("MP3"), FileCat::Audio);
        assert_eq!(FileCat::from_ext("mkv"), FileCat::Video);
        assert_eq!(FileCat::from_ext("dll"), FileCat::Program);
        assert_eq!(FileCat::from_ext(""), FileCat::Other);
        assert_eq!(FileCat::from_ext("sehrlangeendung"), FileCat::Other);
    }

    #[test]
    fn fonts_fall_back_in_order() {
        // Nichts lesbar: egui-Standard unverändert.
        let (defs, ch) = font_definitions(&|_| None);
        assert_eq!(ch, FontChoice::default());
        assert_eq!(defs, FontDefinitions::default());
        // Alles da: Bahnschrift + Consolas vorne.
        let good = test_font();
        let (defs, ch) = font_definitions(&|_| Some(good.clone()));
        assert_eq!(ch.title, Some("bahnschrift.ttf"));
        assert_eq!(ch.mono, Some("consola.ttf"));
        assert_eq!(defs.families[&FontFamily::Proportional][0], FONT_TITLE);
        assert_eq!(defs.families[&FontFamily::Monospace][0], FONT_MONO);
        // egui-Standard bleibt als Rückfall dahinter.
        assert!(defs.families[&FontFamily::Proportional].len() > 1);
        // Ohne Bahnschrift: Segoe UI.
        let (_, ch) = font_definitions(&|f| (f == "segoeui.ttf").then(|| good.clone()));
        assert_eq!(ch.title, Some("segoeui.ttf"));
        assert_eq!(ch.mono, Some("segoeui.ttf"));
    }

    /// Gültige Schrift ohne Windows-Pfad: die in egui eingebettete Standardschrift.
    fn test_font() -> Vec<u8> {
        let defs = FontDefinitions::default();
        let name = &defs.families[&FontFamily::Proportional][0];
        defs.font_data[name].bytes().to_vec()
    }

    #[test]
    fn font_validation_accepts_good_rejects_broken() {
        let good = test_font();
        assert!(font_bytes_valid(&good));
        // Auch die eingebettete Monospace-Standardschrift.
        let defs = FontDefinitions::default();
        let mono = &defs.families[&FontFamily::Monospace][0];
        assert!(font_bytes_valid(defs.font_data[mono].bytes()));
        // Leer, Zufallsbytes, abgeschnitten (Kopf ganz, Tabellen fehlen; halb).
        assert!(!font_bytes_valid(&[]));
        let mut x: u32 = 0x1234_5678;
        let noise: Vec<u8> = (0..4096)
            .map(|_| {
                x ^= x << 13;
                x ^= x >> 17;
                x ^= x << 5;
                x as u8
            })
            .collect();
        assert!(!font_bytes_valid(&noise));
        assert!(!font_bytes_valid(&good[..512.min(good.len())]));
        assert!(!font_bytes_valid(&good[..good.len() / 2]));
        assert!(!font_bytes_valid(&good[..good.len() - 1]));
    }

    #[test]
    fn invalid_font_files_are_skipped() {
        let good = test_font();
        // Bahnschrift kaputt, Segoe UI gültig: Segoe UI wird genommen.
        // Consolas leer: Mono fällt auf Segoe UI zurück.
        let read = |f: &str| match f {
            "bahnschrift.ttf" => Some(vec![0u8; 64]),
            "consola.ttf" => Some(Vec::new()),
            "segoeui.ttf" => Some(good.clone()),
            _ => None,
        };
        let (_, ch) = font_definitions(&read);
        assert_eq!(ch.title, Some("segoeui.ttf"));
        assert_eq!(ch.mono, Some("segoeui.ttf"));
        // Alles kaputt: egui-Standard unverändert.
        let (defs, ch) = font_definitions(&|_| Some(b"kaputt".to_vec()));
        assert_eq!(ch, FontChoice::default());
        assert_eq!(defs, FontDefinitions::default());
    }

    /// Echte Systemschriften (falls vorhanden) werden akzeptiert, und epaint lädt
    /// sie wirklich ohne Panik, auch variable Schriften (wie Bahnschrift).
    #[test]
    fn system_fonts_validate_and_load_in_epaint() {
        let pick = |dir: &str, only_variable: bool, n: usize| -> Vec<std::path::PathBuf> {
            let mut v: Vec<_> = std::fs::read_dir(dir)
                .map(|rd| rd.flatten().map(|e| e.path()).collect())
                .unwrap_or_default();
            v.retain(|p: &std::path::PathBuf| {
                let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
                (name.ends_with(".ttf") || name.ends_with(".otf"))
                    && (!only_variable || name.contains("Variable"))
            });
            v.sort();
            v.truncate(n);
            v
        };
        let mut files = pick("/usr/share/fonts/truetype/dejavu", false, 2);
        files.extend(pick("/usr/share/fonts/opentype/inter", false, 2));
        files.extend(pick("/usr/share/fonts/truetype/google-fonts", true, 3));
        for p in &files {
            let bytes = std::fs::read(p).unwrap();
            assert!(font_bytes_valid(&bytes), "{p:?}");
            let (defs, ch) = font_definitions(&|_| Some(bytes.clone()));
            assert!(ch.title.is_some() && ch.mono.is_some());
            // Familien werden in epaint erst beim ersten Text geladen: erzwingen.
            let mut fonts =
                egui::epaint::text::Fonts::new(egui::epaint::text::TextOptions::default(), defs);
            let mut view = fonts.with_pixels_per_point(1.0);
            for family in [FontFamily::Proportional, FontFamily::Monospace] {
                let g = view.layout_no_wrap(
                    "Größe ÄÖÜ 123".into(),
                    FontId::new(14.0, family),
                    Color32::WHITE,
                );
                assert!(g.size().x > 0.0, "{p:?}");
            }
        }
    }
}
