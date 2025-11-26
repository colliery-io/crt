//! Font configuration helpers for glyphon
//!
//! Creates glyphon Attrs from the config's FontConfig with proper fallback support.

use crt_core::FontConfig;
use glyphon::{Attrs, Family, Style, Weight};

/// Create font family from config, using the primary font with system fallback
pub fn family_from_config(config: &FontConfig) -> Family<'static> {
    // Use the primary font family name
    // cosmic_text will automatically fall back to system fonts if glyphs are missing
    // The family name is leaked to get 'static lifetime (acceptable for fonts)
    Family::Name(Box::leak(config.family.clone().into_boxed_str()))
}

/// Create attrs for regular text from font config
pub fn attrs_from_config(config: &FontConfig) -> Attrs<'static> {
    Attrs::new()
        .family(family_from_config(config))
        .weight(Weight::NORMAL)
        .style(Style::Normal)
}

/// Create attrs for bold text from font config
pub fn attrs_bold_from_config(config: &FontConfig) -> Attrs<'static> {
    Attrs::new()
        .family(family_from_config(config))
        .weight(Weight::BOLD)
        .style(Style::Normal)
}

/// Create attrs for italic text from font config
pub fn attrs_italic_from_config(config: &FontConfig) -> Attrs<'static> {
    Attrs::new()
        .family(family_from_config(config))
        .weight(Weight::NORMAL)
        .style(Style::Italic)
}

/// Create attrs for bold+italic text from font config
pub fn attrs_bold_italic_from_config(config: &FontConfig) -> Attrs<'static> {
    Attrs::new()
        .family(family_from_config(config))
        .weight(Weight::BOLD)
        .style(Style::Italic)
}

/// Helper struct to hold all font attrs needed for terminal rendering
#[derive(Clone)]
pub struct TerminalFontAttrs {
    pub regular: Attrs<'static>,
    pub bold: Attrs<'static>,
    pub italic: Attrs<'static>,
    pub bold_italic: Attrs<'static>,
}

impl TerminalFontAttrs {
    /// Create terminal font attrs from config
    pub fn from_config(config: &FontConfig) -> Self {
        Self {
            regular: attrs_from_config(config),
            bold: attrs_bold_from_config(config),
            italic: attrs_italic_from_config(config),
            bold_italic: attrs_bold_italic_from_config(config),
        }
    }

    /// Get attrs for given cell flags
    ///
    /// Cell flags from alacritty_terminal indicate bold, italic, etc.
    pub fn for_style(&self, bold: bool, italic: bool) -> &Attrs<'static> {
        match (bold, italic) {
            (true, true) => &self.bold_italic,
            (true, false) => &self.bold,
            (false, true) => &self.italic,
            (false, false) => &self.regular,
        }
    }
}

impl Default for TerminalFontAttrs {
    fn default() -> Self {
        Self::from_config(&FontConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_attrs() {
        let attrs = TerminalFontAttrs::default();
        // Just verify it doesn't panic
        let _ = attrs.regular;
        let _ = attrs.bold;
        let _ = attrs.italic;
        let _ = attrs.bold_italic;
    }

    #[test]
    fn test_attrs_from_config() {
        let config = FontConfig {
            family: "Fira Code".to_string(),
            ..Default::default()
        };
        let attrs = TerminalFontAttrs::from_config(&config);
        let _ = attrs.for_style(false, false);
        let _ = attrs.for_style(true, false);
        let _ = attrs.for_style(false, true);
        let _ = attrs.for_style(true, true);
    }
}
