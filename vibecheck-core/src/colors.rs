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
    fn svg_color(&self, family: ModelFamily) -> &'static str;
}

/// The default color theme — matches the original hardcoded palette.
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

    fn svg_color(&self, family: ModelFamily) -> &'static str {
        match family {
            ModelFamily::Claude  => "#d2a8ff",
            ModelFamily::Gpt     => "#7ee787",
            ModelFamily::Gemini  => "#79c0ff",
            ModelFamily::Copilot => "#39c5cf",
            ModelFamily::Human   => "#e3b341",
        }
    }
}

impl ModelFamily {
    /// Hex color used in SVG output (delegates to [`DefaultTheme`]).
    pub fn svg_color(self) -> &'static str {
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
}
