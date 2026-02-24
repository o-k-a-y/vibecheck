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
