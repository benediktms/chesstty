use ratatui::style::Color;

/// All colors used by the TUI, grouped by purpose.
/// Swap between presets (Dark / Light) to adapt to the terminal background.
#[derive(Debug, Clone)]
#[allow(dead_code)] // fields and variants used progressively as theme migration completes
pub struct Theme {
    // ── Board ──────────────────────────────────────────────────────
    pub light_square: Color,
    pub dark_square: Color,
    pub white_piece: Color,
    pub black_piece: Color,
    pub board_border: Color,
    pub board_label: Color,

    // ── Overlays (light-square variant, dark-square variant) ──────
    pub overlay_selected: (Color, Color),
    pub overlay_legal_move: (Color, Color),
    pub overlay_last_move: (Color, Color),
    pub overlay_best_move: (Color, Color),
    pub overlay_typeahead: (Color, Color),
    pub overlay_blunder: (Color, Color),
    pub overlay_brilliant: (Color, Color),
    pub overlay_danger: (Color, Color),
    pub overlay_tactical: (Color, Color),

    // ── Panel chrome ──────────────────────────────────────────────
    pub panel_border: Color,
    pub panel_border_selected: Color,
    pub panel_border_dimmed: Color,
    pub panel_border_debug: Color,
    pub panel_border_review: Color,

    // ── Semantic status ───────────────────────────────────────────
    pub positive: Color,
    pub positive_light: Color,
    pub warning: Color,
    pub negative: Color,
    pub negative_light: Color,
    pub info: Color,
    pub info_light: Color,
    pub secondary: Color,
    pub secondary_light: Color,
    pub muted: Color,

    // ── Text ──────────────────────────────────────────────────────
    pub text_primary: Color,
    pub text_secondary: Color,
    pub text_bright: Color,

    // ── Dialogs / menus ───────────────────────────────────────────
    pub dialog_bg: Color,
    pub dialog_border: Color,
    pub dialog_highlight: Color,
    pub dialog_highlight_bg: Color,

    // ── Move classification ───────────────────────────────────────
    pub move_brilliant: Color,
    pub move_excellent: Color,
    pub move_good: Color,
    pub move_inaccuracy: Color,
    pub move_mistake: Color,
    pub move_blunder: Color,
    pub move_forced: Color,

    // ── Evaluation ────────────────────────────────────────────────
    pub eval_positive: Color,
    pub eval_negative: Color,
    pub eval_equal: Color,
    pub eval_mate_positive: Color,
    pub eval_mate_negative: Color,

    // ── Accuracy thresholds ───────────────────────────────────────
    pub accuracy_high: Color,
    pub accuracy_mid: Color,
    pub accuracy_low: Color,
}

impl Theme {
    /// Dark theme — matches the original hardcoded colors exactly.
    /// Designed for terminals with a dark background.
    pub fn dark() -> Self {
        Self {
            // Board
            light_square: Color::Rgb(240, 217, 181),
            dark_square: Color::Rgb(181, 136, 99),
            // Use explicit RGB instead of ANSI White/Black — many terminals
            // remap ANSI colors, causing pieces to blend into the board.
            white_piece: Color::Rgb(255, 255, 255),
            black_piece: Color::Rgb(0, 0, 0),
            board_border: Color::Cyan,
            board_label: Color::Yellow,

            // Overlays
            overlay_selected: (Color::LightYellow, Color::Yellow),
            overlay_legal_move: (Color::LightBlue, Color::Blue),
            overlay_last_move: (Color::LightYellow, Color::Yellow),
            overlay_best_move: (Color::LightGreen, Color::Green),
            overlay_typeahead: (Color::LightCyan, Color::Cyan),
            overlay_blunder: (Color::LightRed, Color::Red),
            overlay_brilliant: (Color::LightMagenta, Color::Magenta),
            overlay_danger: (Color::LightRed, Color::Red),
            overlay_tactical: (Color::Rgb(255, 200, 100), Color::Rgb(200, 150, 50)),

            // Panel chrome
            panel_border: Color::Cyan,
            panel_border_selected: Color::Yellow,
            panel_border_dimmed: Color::DarkGray,
            panel_border_debug: Color::Magenta,
            panel_border_review: Color::Green,

            // Semantic status
            positive: Color::Green,
            positive_light: Color::LightGreen,
            warning: Color::Yellow,
            negative: Color::Red,
            negative_light: Color::LightRed,
            info: Color::Cyan,
            info_light: Color::LightCyan,
            secondary: Color::Magenta,
            secondary_light: Color::LightMagenta,
            muted: Color::DarkGray,

            // Text
            text_primary: Color::White,
            text_secondary: Color::Gray,
            text_bright: Color::White,

            // Dialogs / menus
            dialog_bg: Color::Black,
            dialog_border: Color::Yellow,
            dialog_highlight: Color::Yellow,
            dialog_highlight_bg: Color::DarkGray,

            // Move classification
            move_brilliant: Color::Cyan,
            move_excellent: Color::Cyan,
            move_good: Color::White,
            move_inaccuracy: Color::Yellow,
            move_mistake: Color::Magenta,
            move_blunder: Color::Red,
            move_forced: Color::DarkGray,

            // Evaluation
            eval_positive: Color::Green,
            eval_negative: Color::Red,
            eval_equal: Color::White,
            eval_mate_positive: Color::LightGreen,
            eval_mate_negative: Color::LightRed,

            // Accuracy
            accuracy_high: Color::Green,
            accuracy_mid: Color::Yellow,
            accuracy_low: Color::Red,
        }
    }

    /// Light theme — designed for terminals with a light background.
    /// Uses explicit RGB to avoid ANSI color remapping, and darker board
    /// squares so white pieces have sufficient contrast.
    pub fn light() -> Self {
        Self {
            // Board — darker squares so Rgb(255,255,255) white pieces pop
            light_square: Color::Rgb(210, 180, 140),
            dark_square: Color::Rgb(150, 110, 70),
            white_piece: Color::Rgb(255, 255, 255),
            black_piece: Color::Rgb(30, 30, 30),
            board_border: Color::Rgb(60, 60, 60),
            board_label: Color::Rgb(100, 80, 50),

            // Overlays — use RGB to avoid ANSI remapping in light terminals
            overlay_selected: (Color::Rgb(220, 200, 80), Color::Rgb(180, 160, 40)),
            overlay_legal_move: (Color::Rgb(100, 160, 220), Color::Rgb(60, 120, 180)),
            overlay_last_move: (Color::Rgb(220, 200, 80), Color::Rgb(180, 160, 40)),
            overlay_best_move: (Color::Rgb(80, 180, 80), Color::Rgb(40, 140, 40)),
            overlay_typeahead: (Color::Rgb(80, 200, 200), Color::Rgb(40, 160, 160)),
            overlay_blunder: (Color::Rgb(220, 80, 80), Color::Rgb(180, 40, 40)),
            overlay_brilliant: (Color::Rgb(200, 100, 220), Color::Rgb(160, 60, 180)),
            overlay_danger: (Color::Rgb(220, 80, 80), Color::Rgb(180, 40, 40)),
            overlay_tactical: (Color::Rgb(220, 170, 70), Color::Rgb(180, 130, 30)),

            // Panel chrome — dark borders on light background
            panel_border: Color::Rgb(60, 120, 140),
            panel_border_selected: Color::Rgb(160, 130, 30),
            panel_border_dimmed: Color::Rgb(180, 180, 180),
            panel_border_debug: Color::Rgb(140, 60, 140),
            panel_border_review: Color::Rgb(40, 120, 40),

            // Semantic status — darker shades for light bg readability
            positive: Color::Rgb(30, 140, 30),
            positive_light: Color::Rgb(60, 180, 60),
            warning: Color::Rgb(180, 140, 0),
            negative: Color::Rgb(200, 40, 40),
            negative_light: Color::Rgb(220, 80, 80),
            info: Color::Rgb(30, 120, 150),
            info_light: Color::Rgb(60, 160, 190),
            secondary: Color::Rgb(140, 50, 140),
            secondary_light: Color::Rgb(180, 80, 180),
            muted: Color::Rgb(150, 150, 150),

            // Text — dark on light
            text_primary: Color::Rgb(30, 30, 30),
            text_secondary: Color::Rgb(100, 100, 100),
            text_bright: Color::Rgb(0, 0, 0),

            // Dialogs / menus — light backgrounds with dark text
            dialog_bg: Color::Rgb(245, 245, 240),
            dialog_border: Color::Rgb(160, 130, 30),
            dialog_highlight: Color::Rgb(160, 130, 30),
            dialog_highlight_bg: Color::Rgb(220, 220, 210),

            // Move classification
            move_brilliant: Color::Rgb(30, 120, 150),
            move_excellent: Color::Rgb(30, 120, 150),
            move_good: Color::Rgb(30, 30, 30),
            move_inaccuracy: Color::Rgb(180, 140, 0),
            move_mistake: Color::Rgb(140, 50, 140),
            move_blunder: Color::Rgb(200, 40, 40),
            move_forced: Color::Rgb(150, 150, 150),

            // Evaluation
            eval_positive: Color::Rgb(30, 140, 30),
            eval_negative: Color::Rgb(200, 40, 40),
            eval_equal: Color::Rgb(30, 30, 30),
            eval_mate_positive: Color::Rgb(60, 180, 60),
            eval_mate_negative: Color::Rgb(220, 80, 80),

            // Accuracy
            accuracy_high: Color::Rgb(30, 140, 30),
            accuracy_mid: Color::Rgb(180, 140, 0),
            accuracy_low: Color::Rgb(200, 40, 40),
        }
    }

    /// Detect theme from the `CHESSTTY_THEME` environment variable.
    ///
    /// Set `CHESSTTY_THEME=light` for light terminals. Defaults to dark.
    pub fn detect() -> Self {
        Self::from_preference(std::env::var("CHESSTTY_THEME").ok().as_deref())
    }

    /// Create a theme from a preference string ("light" or "dark").
    /// Returns dark for any unrecognized or `None` value.
    pub fn from_preference(pref: Option<&str>) -> Self {
        match pref {
            Some("light") => Self::light(),
            _ => Self::dark(),
        }
    }

    /// Toggle between dark and light theme in place.
    pub fn toggle(&mut self) {
        *self = if self.is_dark() {
            Self::light()
        } else {
            Self::dark()
        };
    }

    /// Returns true if this is the dark theme variant.
    pub fn is_dark(&self) -> bool {
        // Dark theme uses ANSI Color::White for text_primary;
        // light theme uses Rgb(30,30,30).
        self.text_primary == Color::White
    }

    /// Human-readable name of the current theme variant.
    #[allow(dead_code)] // used in tests and available for status display
    pub fn name(&self) -> &'static str {
        if self.is_dark() {
            "Dark"
        } else {
            "Light"
        }
    }

    /// Resolve an overlay color pair for a given square.
    #[allow(dead_code)] // public API for overlay resolution
    pub fn resolve_overlay(&self, overlay: OverlayKind, is_light_square: bool) -> Color {
        let (light, dark) = match overlay {
            OverlayKind::Selected => self.overlay_selected,
            OverlayKind::LegalMove => self.overlay_legal_move,
            OverlayKind::LastMove => self.overlay_last_move,
            OverlayKind::BestMove => self.overlay_best_move,
            OverlayKind::Typeahead => self.overlay_typeahead,
            OverlayKind::Blunder => self.overlay_blunder,
            OverlayKind::Brilliant => self.overlay_brilliant,
            OverlayKind::Danger => self.overlay_danger,
            OverlayKind::Tactical => self.overlay_tactical,
        };
        if is_light_square {
            light
        } else {
            dark
        }
    }
}

/// Overlay types for theme resolution (mirrors OverlayColor variants minus Custom).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // variants used via resolve_overlay()
pub enum OverlayKind {
    Selected,
    LegalMove,
    LastMove,
    BestMove,
    Typeahead,
    Blunder,
    Brilliant,
    Danger,
    Tactical,
}

impl Default for Theme {
    fn default() -> Self {
        Self::detect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dark_theme_is_dark() {
        let theme = Theme::dark();
        assert!(theme.is_dark());
        assert_eq!(theme.name(), "Dark");
    }

    #[test]
    fn light_theme_is_not_dark() {
        let theme = Theme::light();
        assert!(!theme.is_dark());
        assert_eq!(theme.name(), "Light");
    }

    #[test]
    fn toggle_switches_variant() {
        let mut theme = Theme::dark();
        assert!(theme.is_dark());

        theme.toggle();
        assert!(!theme.is_dark());

        theme.toggle();
        assert!(theme.is_dark());
    }

    #[test]
    fn detect_falls_back_to_dark_for_unknown_values() {
        // detect() returns dark for any value other than "light"
        // (including when CHESSTTY_THEME is unset, which is the common case)
        let theme = Theme::from_preference(Some("dark"));
        assert!(theme.is_dark());

        let theme = Theme::from_preference(Some("bogus"));
        assert!(theme.is_dark());

        let theme = Theme::from_preference(None);
        assert!(theme.is_dark());

        let theme = Theme::from_preference(Some("light"));
        assert!(!theme.is_dark());
    }
}
