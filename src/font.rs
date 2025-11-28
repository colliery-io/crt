//! Font loading and discovery
//!
//! Loads fonts from ~/.config/crt/fonts/ first, then falls back to system fonts.

use crate::config::{Config, FontConfig};
use crt_renderer::FontVariants;
use fontdb::{Database, Family, Query, Style, Weight};
use std::path::PathBuf;
use std::sync::OnceLock;

/// Global font database (loaded once)
static FONT_DB: OnceLock<Database> = OnceLock::new();

/// Get the fonts directory path (~/.config/crt/fonts)
fn fonts_dir() -> Option<PathBuf> {
    Config::config_dir().map(|p| p.join("fonts"))
}

/// Get or initialize the font database
fn font_db() -> &'static Database {
    FONT_DB.get_or_init(|| {
        let mut db = Database::new();

        // Load fonts from config directory first
        if let Some(fonts_path) = fonts_dir() {
            if fonts_path.exists() {
                db.load_fonts_dir(&fonts_path);
                log::info!("Loaded fonts from {:?}", fonts_path);
            }
        }

        // Then load system fonts as fallback
        db.load_system_fonts();
        log::info!(
            "Font database initialized with {} fonts",
            db.faces().count()
        );
        db
    })
}

/// Load font data by family name and style
fn load_font(family: &str, weight: Weight, style: Style) -> Option<Vec<u8>> {
    let db = font_db();

    let query = Query {
        families: &[Family::Name(family)],
        weight,
        style,
        ..Default::default()
    };

    let face_id = db.query(&query)?;
    let face = db.face(face_id)?;

    // fontdb gives us the font source - we need to read the data
    match &face.source {
        fontdb::Source::File(path) => std::fs::read(path).ok(),
        fontdb::Source::Binary(data) => Some(data.as_ref().as_ref().to_vec()),
        fontdb::Source::SharedFile(_path, data) => Some(data.as_ref().as_ref().to_vec()),
    }
}

/// Try to load a font from a list of family names (first match wins)
fn load_font_from_families(families: &[String], weight: Weight, style: Style) -> Option<Vec<u8>> {
    for family in families {
        if let Some(data) = load_font(family, weight, style) {
            log::info!("Loaded font: {} ({:?}, {:?})", family, weight, style);
            return Some(data);
        }
    }
    None
}

/// Load font variants based on config
///
/// Looks for fonts in:
/// 1. ~/.config/crt/fonts/ (installed by installer)
/// 2. System fonts
///
/// Falls back to MesloLGS NF from config dir, then any available monospace font.
pub fn load_font_variants(config: &FontConfig) -> FontVariants {
    // Try to load regular font from config families
    let regular = load_font_from_families(&config.family, Weight::NORMAL, Style::Normal)
        .or_else(|| {
            // Fallback: try MesloLGS NF from config fonts dir
            log::info!("Configured font not found, trying MesloLGS NF");
            load_font("MesloLGS NF", Weight::NORMAL, Style::Normal)
        })
        .or_else(|| {
            // Last resort: try common system monospace fonts
            log::warn!("MesloLGS NF not found - install fonts to ~/.config/crt/fonts/");
            load_font_from_families(
                &["Menlo", "Monaco", "Consolas", "DejaVu Sans Mono"]
                    .iter()
                    .map(|s| s.to_string())
                    .collect::<Vec<_>>(),
                Weight::NORMAL,
                Style::Normal,
            )
        })
        .expect("No suitable font found. Please install fonts to ~/.config/crt/fonts/");

    // Try to load bold/italic variants from same family, fall back to regular
    let bold = load_font_from_families(&config.family, Weight::BOLD, Style::Normal)
        .or_else(|| load_font("MesloLGS NF", Weight::BOLD, Style::Normal))
        .unwrap_or_else(|| regular.clone());

    let italic = load_font_from_families(&config.family, Weight::NORMAL, Style::Italic)
        .or_else(|| load_font("MesloLGS NF", Weight::NORMAL, Style::Italic))
        .unwrap_or_else(|| regular.clone());

    let bold_italic = load_font_from_families(&config.family, Weight::BOLD, Style::Italic)
        .or_else(|| load_font("MesloLGS NF", Weight::BOLD, Style::Italic))
        .unwrap_or_else(|| regular.clone());

    FontVariants::new(regular)
        .with_bold(bold)
        .with_italic(italic)
        .with_bold_italic(bold_italic)
}

/// List available monospace fonts (for debugging/config help)
#[allow(dead_code)]
pub fn list_monospace_fonts() -> Vec<String> {
    let db = font_db();
    let mut fonts = Vec::new();

    for face in db.faces() {
        if face.monospaced {
            let family = face.families.first().map(|(name, _)| name.clone());
            if let Some(name) = family {
                if !fonts.contains(&name) {
                    fonts.push(name);
                }
            }
        }
    }

    fonts.sort();
    fonts
}
