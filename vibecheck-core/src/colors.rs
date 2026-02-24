use crate::report::ModelFamily;

/// Dependency-injection seam for model-family color mapping.
///
/// Implement this trait to provide custom color themes (e.g. high-contrast or
/// colour-blind-friendly palettes). The built-in implementation is
/// [`DefaultTheme`].
pub trait ColorTheme: Send + Sync {
    /// Terminal color name accepted by the `colored` crate (e.g. `"magenta"`).
    fn terminal_color(&self, family: ModelFamily) -> &'static str;
    /// Hex color string for SVG output (e.g. `"#d2a8ff"`).
    /// Returns an owned `String` so implementations can derive the value
    /// from [`ModelFamily::rgb`] without needing a static lookup table.
    fn svg_color(&self, family: ModelFamily) -> String;
}

/// The default color theme — derives all colors from the canonical
/// [`ModelFamily::rgb`] values so there is a single source of truth.
pub struct DefaultTheme;

impl ColorTheme for DefaultTheme {
    fn terminal_color(&self, family: ModelFamily) -> &'static str {
        match family {
            ModelFamily::Claude  => "magenta",
            ModelFamily::Gpt     => "green",
            ModelFamily::Gemini  => "blue",
            ModelFamily::Copilot => "cyan",
            ModelFamily::Human   => "yellow",
        }
    }

    fn svg_color(&self, family: ModelFamily) -> String {
        let (r, g, b) = family.rgb();
        format!("#{r:02x}{g:02x}{b:02x}")
    }
}

impl ModelFamily {
    /// Canonical RGB color for this model family — the **single source of truth**
    /// for all color consumers (TUI, SVG, web, etc.).
    pub fn rgb(self) -> (u8, u8, u8) {
        match self {
            ModelFamily::Claude  => (210, 168, 255), // purple
            ModelFamily::Gpt     => (126, 231, 135), // green
            ModelFamily::Gemini  => (121, 192, 255), // blue
            ModelFamily::Copilot => ( 57, 197, 207), // teal
            ModelFamily::Human   => (227, 179,  65), // gold
        }
    }

    /// Hex color used in SVG output — derived from [`Self::rgb`].
    pub fn svg_color(self) -> String {
        DefaultTheme.svg_color(self)
    }

    /// Terminal color name for use with the `colored` crate (delegates to [`DefaultTheme`]).
    pub fn terminal_color(self) -> &'static str {
        DefaultTheme.terminal_color(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn svg_colors_all_families() {
        assert_eq!(ModelFamily::Claude.svg_color(),  "#d2a8ff");
        assert_eq!(ModelFamily::Gpt.svg_color(),     "#7ee787");
        assert_eq!(ModelFamily::Gemini.svg_color(),  "#79c0ff");
        assert_eq!(ModelFamily::Copilot.svg_color(), "#39c5cf");
        assert_eq!(ModelFamily::Human.svg_color(),   "#e3b341");
    }

    #[test]
    fn terminal_colors_all_families() {
        assert_eq!(ModelFamily::Claude.terminal_color(),  "magenta");
        assert_eq!(ModelFamily::Gpt.terminal_color(),     "green");
        assert_eq!(ModelFamily::Gemini.terminal_color(),  "blue");
        assert_eq!(ModelFamily::Copilot.terminal_color(), "cyan");
        assert_eq!(ModelFamily::Human.terminal_color(),   "yellow");
    }

    #[test]
    fn rgb_round_trips_to_svg() {
        for family in [
            ModelFamily::Claude,
            ModelFamily::Gpt,
            ModelFamily::Gemini,
            ModelFamily::Copilot,
            ModelFamily::Human,
        ] {
            let (r, g, b) = family.rgb();
            let expected = format!("#{r:02x}{g:02x}{b:02x}");
            assert_eq!(family.svg_color(), expected);
        }
    }

    #[test]
    fn abbrev_nonempty_for_all_families() {
        for family in [
            ModelFamily::Claude,
            ModelFamily::Gpt,
            ModelFamily::Gemini,
            ModelFamily::Copilot,
            ModelFamily::Human,
        ] {
            assert!(!family.abbrev().is_empty());
        }
    }
}
