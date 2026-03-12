//! Basic types and utility functions for window state.

use crt_core::AnsiColor;
use crt_theme::AnsiPalette;

/// Unique identifier for a terminal tab
pub type TabId = u64;

/// Compile-time-safe identifier for backdrop effects.
///
/// Replaces magic string literals like `"starfield"` and `"sprite"` with
/// enum variants that the compiler can check exhaustively.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EffectId {
    Starfield,
    Particles,
    Grid,
    Rain,
    Matrix,
    Shape,
    Sprite,
}

impl EffectId {
    /// Convert to the string key expected by the renderer crate.
    pub fn as_str(self) -> &'static str {
        match self {
            EffectId::Starfield => "starfield",
            EffectId::Particles => "particles",
            EffectId::Grid => "grid",
            EffectId::Rain => "rain",
            EffectId::Matrix => "matrix",
            EffectId::Shape => "shape",
            EffectId::Sprite => "sprite",
        }
    }
}

/// Map alacritty_terminal AnsiColor to RGBA array using theme palette
pub(crate) fn ansi_color_to_rgba(
    color: AnsiColor,
    palette: &AnsiPalette,
    default_fg: [f32; 4],
    default_bg: [f32; 4],
) -> [f32; 4] {
    use crt_core::AnsiColor::*;
    use crt_core::NamedColor;

    match color {
        // Named colors (0-7 normal, 8-15 bright)
        Named(named) => {
            let c = match named {
                NamedColor::Black => palette.black,
                NamedColor::Red => palette.red,
                NamedColor::Green => palette.green,
                NamedColor::Yellow => palette.yellow,
                NamedColor::Blue => palette.blue,
                NamedColor::Magenta => palette.magenta,
                NamedColor::Cyan => palette.cyan,
                NamedColor::White => palette.white,
                NamedColor::BrightBlack => palette.bright_black,
                NamedColor::BrightRed => palette.bright_red,
                NamedColor::BrightGreen => palette.bright_green,
                NamedColor::BrightYellow => palette.bright_yellow,
                NamedColor::BrightBlue => palette.bright_blue,
                NamedColor::BrightMagenta => palette.bright_magenta,
                NamedColor::BrightCyan => palette.bright_cyan,
                NamedColor::BrightWhite => palette.bright_white,
                // Foreground/Background use their actual theme colors
                NamedColor::Foreground => return default_fg,
                NamedColor::Background => return default_bg,
                // Dim variants use regular colors
                NamedColor::DimBlack => palette.black,
                NamedColor::DimRed => palette.red,
                NamedColor::DimGreen => palette.green,
                NamedColor::DimYellow => palette.yellow,
                NamedColor::DimBlue => palette.blue,
                NamedColor::DimMagenta => palette.magenta,
                NamedColor::DimCyan => palette.cyan,
                NamedColor::DimWhite => palette.white,
                // Cursor color - use foreground as default
                NamedColor::Cursor => return default_fg,
                // Bright foreground
                NamedColor::BrightForeground => palette.bright_white,
                NamedColor::DimForeground => palette.white,
            };
            c.to_array()
        }
        // Indexed colors (0-255)
        Indexed(idx) => {
            if idx < 16 {
                // First 16 are the base ANSI palette
                palette.get(idx).to_array()
            } else {
                // Extended colors (16-255): check for theme override first
                if let Some(color) = palette.get_extended(idx) {
                    color.to_array()
                } else {
                    // Fall back to calculated standard 256-color palette
                    AnsiPalette::calculate_extended(idx).to_array()
                }
            }
        }
        // Direct RGB color
        Spec(rgb) => [
            rgb.r as f32 / 255.0,
            rgb.g as f32 / 255.0,
            rgb.b as f32 / 255.0,
            1.0,
        ],
    }
}
