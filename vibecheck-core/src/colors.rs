use crate::report::ModelFamily;

impl ModelFamily {
    /// Hex color used in SVG output.
    pub fn svg_color(self) -> &'static str {
        match self {
            ModelFamily::Claude  => "#d2a8ff",
            ModelFamily::Gpt     => "#7ee787",
            ModelFamily::Gemini  => "#79c0ff",
            ModelFamily::Copilot => "#39c5cf",
            ModelFamily::Human   => "#e3b341",
        }
    }

    /// Terminal color name for use with the `colored` crate.
    pub fn terminal_color(self) -> &'static str {
        match self {
            ModelFamily::Claude  => "magenta",
            ModelFamily::Gpt     => "green",
            ModelFamily::Gemini  => "blue",
            ModelFamily::Copilot => "cyan",
            ModelFamily::Human   => "yellow",
        }
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
