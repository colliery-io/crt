//! Font loading and discovery
//!
//! Uses fontdb to find system fonts by family name with fallback support.

use crate::config::FontConfig;
use crt_renderer::FontVariants;
use fontdb::{Database, Family, Query, Style, Weight};
use std::sync::OnceLock;

// Embedded fonts as fallback
const EMBEDDED_REGULAR: &[u8] = include_bytes!("../assets/fonts/MesloLGS-NF-Regular.ttf");
const EMBEDDED_BOLD: &[u8] = include_bytes!("../assets/fonts/MesloLGS-NF-Bold.ttf");
const EMBEDDED_ITALIC: &[u8] = include_bytes!("../assets/fonts/MesloLGS-NF-Italic.ttf");
const EMBEDDED_BOLD_ITALIC: &[u8] = include_bytes!("../assets/fonts/MesloLGS-NF-BoldItalic.ttf");

/// Global font database (loaded once)
static FONT_DB: OnceLock<Database> = OnceLock::new();

/// Get or initialize the font database
fn font_db() -> &'static Database {
    FONT_DB.get_or_init(|| {
        let mut db = Database::new();
        db.load_system_fonts();
        log::info!("Loaded {} system fonts", db.faces().count());
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
        fontdb::Source::File(path) => {
            std::fs::read(path).ok()
        }
        fontdb::Source::Binary(data) => {
            Some(data.as_ref().as_ref().to_vec())
        }
        fontdb::Source::SharedFile(_path, data) => {
            Some(data.as_ref().as_ref().to_vec())
        }
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

/// Load font variants based on config, falling back to embedded fonts
pub fn load_font_variants(config: &FontConfig) -> FontVariants {
    // Try to load regular font from config families
    let regular = load_font_from_families(&config.family, Weight::NORMAL, Style::Normal)
        .unwrap_or_else(|| {
            log::info!("Using embedded font (no system font found)");
            EMBEDDED_REGULAR.to_vec()
        });

    // Try to load bold/italic variants from same family
    let bold = load_font_from_families(&config.family, Weight::BOLD, Style::Normal)
        .unwrap_or_else(|| EMBEDDED_BOLD.to_vec());

    let italic = load_font_from_families(&config.family, Weight::NORMAL, Style::Italic)
        .unwrap_or_else(|| EMBEDDED_ITALIC.to_vec());

    let bold_italic = load_font_from_families(&config.family, Weight::BOLD, Style::Italic)
        .unwrap_or_else(|| EMBEDDED_BOLD_ITALIC.to_vec());

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
