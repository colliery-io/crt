//! Bundled default themes
//!
//! These themes are embedded in the binary and can be used without
//! any external files.

/// Bundled theme data
pub struct BundledTheme {
    pub name: &'static str,
    pub css: &'static str,
}

/// Synthwave theme - neon colors with optional grid effect
pub const SYNTHWAVE: BundledTheme = BundledTheme {
    name: "synthwave",
    css: include_str!("../../../themes/synthwave.css"),
};

/// Minimal theme - clean and simple
pub const MINIMAL: BundledTheme = BundledTheme {
    name: "minimal",
    css: include_str!("../../../themes/minimal.css"),
};

/// Dracula theme - dark and elegant
pub const DRACULA: BundledTheme = BundledTheme {
    name: "dracula",
    css: include_str!("../../../themes/dracula.css"),
};

/// Solarized dark theme - precision colors for readability
pub const SOLARIZED: BundledTheme = BundledTheme {
    name: "solarized",
    css: include_str!("../../../themes/solarized.css"),
};

/// All bundled themes
pub const ALL_THEMES: &[&BundledTheme] = &[
    &SYNTHWAVE,
    &MINIMAL,
    &DRACULA,
    &SOLARIZED,
];

/// Get a bundled theme by name
pub fn get_bundled_theme(name: &str) -> Option<&'static BundledTheme> {
    ALL_THEMES.iter().find(|t| t.name == name).copied()
}

/// List all available bundled theme names
pub fn bundled_theme_names() -> Vec<&'static str> {
    ALL_THEMES.iter().map(|t| t.name).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bundled_themes_exist() {
        assert!(!SYNTHWAVE.css.is_empty());
        assert!(!MINIMAL.css.is_empty());
        assert!(!DRACULA.css.is_empty());
        assert!(!SOLARIZED.css.is_empty());
    }

    #[test]
    fn test_get_bundled_theme() {
        assert!(get_bundled_theme("synthwave").is_some());
        assert!(get_bundled_theme("minimal").is_some());
        assert!(get_bundled_theme("dracula").is_some());
        assert!(get_bundled_theme("solarized").is_some());
        assert!(get_bundled_theme("nonexistent").is_none());
    }

    #[test]
    fn test_bundled_theme_names() {
        let names = bundled_theme_names();
        assert!(names.contains(&"synthwave"));
        assert!(names.contains(&"minimal"));
        assert!(names.contains(&"dracula"));
        assert!(names.contains(&"solarized"));
    }
}
