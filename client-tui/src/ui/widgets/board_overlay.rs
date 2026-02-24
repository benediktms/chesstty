use crate::review_state::ReviewState;
use chess_client;
use cozy_chess::Square;
use ratatui::style::Color;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[allow(dead_code)]
pub enum Layer {
    #[default]
    Board = 0,
    Highlights = 1,
    Pieces = 2,
}

/// Semantic overlay colors that the board widget maps to terminal colors.
/// Each variant has light/dark square variants for contrast.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum OverlayColor {
    /// Yellow — piece selection
    Selected,
    /// Blue — legal move destinations
    LegalMove,
    /// Yellow — previous move from/to
    LastMove,
    /// Green — engine best move recommendation
    BestMove,
    /// Cyan — typeahead input match
    Typeahead,
    /// Red — blunder highlight
    Blunder,
    /// Magenta — brilliant move
    Brilliant,
    /// Red — danger zone (king safety threats)
    Danger,
    /// Yellow/orange — tactical pattern
    Tactical,
    /// Escape hatch for arbitrary colors (light_square, dark_square)
    Custom(Color, Color),
}

impl OverlayColor {
    /// Resolve to a terminal color based on whether the square is light or dark.
    pub fn resolve(self, is_light_square: bool) -> Color {
        let (light, dark) = match self {
            Self::Selected => (Color::LightYellow, Color::Yellow),
            Self::LegalMove => (Color::LightBlue, Color::Blue),
            Self::LastMove => (Color::LightYellow, Color::Yellow),
            Self::BestMove => (Color::LightGreen, Color::Green),
            Self::Typeahead => (Color::LightCyan, Color::Cyan),
            Self::Blunder => (Color::LightRed, Color::Red),
            Self::Brilliant => (Color::LightMagenta, Color::Magenta),
            Self::Danger => (Color::LightRed, Color::Red),
            Self::Tactical => (Color::Rgb(255, 200, 100), Color::Rgb(200, 150, 50)),
            Self::Custom(l, d) => (l, d),
        };
        if is_light_square {
            light
        } else {
            dark
        }
    }
}

/// A single visual element to draw on the board.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum OverlayElement {
    /// Color a square's background.
    SquareTint {
        square: Square,
        color: OverlayColor,
        layer: Layer,
    },
    /// Draw a border/outline around a square.
    SquareOutline {
        square: Square,
        color: OverlayColor,
        layer: Layer,
    },
    /// Draw an arrow between two squares.
    /// Rendered as tinted from/to squares with a directional marker.
    Arrow {
        from: Square,
        to: Square,
        color: OverlayColor,
        layer: Layer,
    },
}

/// Ordered collection of overlay elements organized by layer.
/// Layers are rendered in order: Board -> Highlights -> Pieces.
#[derive(Debug, Clone, Default)]
pub struct BoardOverlay {
    layers: BTreeMap<Layer, Vec<OverlayElement>>,
}

impl BoardOverlay {
    pub fn new() -> Self {
        Self {
            layers: BTreeMap::new(),
        }
    }

    fn get_or_insert_layer(&mut self, layer: Layer) -> &mut Vec<OverlayElement> {
        self.layers.entry(layer).or_default()
    }

    /// Add a square background tint.
    pub fn tint(&mut self, square: Square, color: OverlayColor) -> &mut Self {
        self.tint_on_layer(square, color, Layer::Highlights)
    }

    /// Add a square background tint on a specific layer.
    pub fn tint_on_layer(
        &mut self,
        square: Square,
        color: OverlayColor,
        layer: Layer,
    ) -> &mut Self {
        self.get_or_insert_layer(layer)
            .push(OverlayElement::SquareTint {
                square,
                color,
                layer,
            });
        self
    }

    /// Add a square outline/border.
    #[allow(dead_code)]
    pub fn outline(&mut self, square: Square, color: OverlayColor) -> &mut Self {
        self.outline_on_layer(square, color, Layer::Highlights)
    }

    /// Add a square outline/border on a specific layer.
    pub fn outline_on_layer(
        &mut self,
        square: Square,
        color: OverlayColor,
        layer: Layer,
    ) -> &mut Self {
        self.get_or_insert_layer(layer)
            .push(OverlayElement::SquareOutline {
                square,
                color,
                layer,
            });
        self
    }

    /// Add an arrow between two squares.
    pub fn arrow(&mut self, from: Square, to: Square, color: OverlayColor) -> &mut Self {
        self.arrow_on_layer(from, to, color, Layer::Highlights)
    }

    /// Add an arrow between two squares on a specific layer.
    pub fn arrow_on_layer(
        &mut self,
        from: Square,
        to: Square,
        color: OverlayColor,
        layer: Layer,
    ) -> &mut Self {
        self.get_or_insert_layer(layer).push(OverlayElement::Arrow {
            from,
            to,
            color,
            layer,
        });
        self
    }

    /// Get the background tint color for a square (last tint wins, considering layer order).
    /// Returns None if no tint is applied to this square.
    pub fn square_tint(&self, square: Square) -> Option<OverlayColor> {
        self.square_tint_on_layer(square, Layer::Highlights)
    }

    /// Get the background tint color for a square on a specific layer.
    pub fn square_tint_on_layer(&self, square: Square, layer: Layer) -> Option<OverlayColor> {
        let mut result = None;
        for (l, elements) in &self.layers {
            if *l > layer {
                break;
            }
            for element in elements {
                match element {
                    OverlayElement::SquareTint {
                        square: sq,
                        color,
                        layer: elem_layer,
                    } if *sq == square && *elem_layer == layer => {
                        result = Some(*color);
                    }
                    OverlayElement::Arrow {
                        from,
                        to,
                        color,
                        layer: elem_layer,
                    } if (*from == square || *to == square) && *elem_layer == layer => {
                        result = Some(*color);
                    }
                    _ => {}
                }
            }
        }
        result
    }

    /// Get outline color for a square (last outline wins, considering layer order).
    pub fn square_outline(&self, square: Square) -> Option<OverlayColor> {
        self.square_outline_on_layer(square, Layer::Highlights)
    }

    /// Get outline color for a square on a specific layer.
    pub fn square_outline_on_layer(&self, square: Square, layer: Layer) -> Option<OverlayColor> {
        let mut result = None;
        for (l, elements) in &self.layers {
            if *l > layer {
                break;
            }
            for element in elements {
                if let OverlayElement::SquareOutline {
                    square: sq,
                    color,
                    layer: elem_layer,
                } = element
                {
                    if *sq == square && *elem_layer == layer {
                        result = Some(*color);
                    }
                }
            }
        }
        result
    }

    /// Get arrow annotations landing on a specific square.
    /// Returns the arrow direction character and color for the destination square.
    #[allow(dead_code)]
    pub fn arrow_annotation(&self, square: Square) -> Option<(&'static str, OverlayColor)> {
        self.arrow_annotation_on_layer(square, Layer::Highlights)
    }

    /// Get arrow annotations on a specific layer.
    #[allow(dead_code)]
    pub fn arrow_annotation_on_layer(
        &self,
        square: Square,
        layer: Layer,
    ) -> Option<(&'static str, OverlayColor)> {
        for (l, elements) in &self.layers {
            if *l > layer {
                break;
            }
            for element in elements.iter().rev() {
                if let OverlayElement::Arrow {
                    from,
                    to,
                    color,
                    layer: elem_layer,
                } = element
                {
                    if *to == square && *elem_layer == layer {
                        let symbol = arrow_symbol(*from, *to);
                        return Some((symbol, *color));
                    }
                }
            }
        }
        None
    }

    /// Get all elements (for iteration), ordered by layer.
    pub fn elements(&self) -> Vec<&OverlayElement> {
        let mut result = Vec::new();
        for elements in self.layers.values() {
            for element in elements {
                result.push(element);
            }
        }
        result
    }

    /// Get elements on a specific layer.
    #[allow(dead_code)]
    pub fn elements_on_layer(&self, layer: Layer) -> Vec<&OverlayElement> {
        self.layers
            .get(&layer)
            .map(|elements| elements.iter().collect())
            .unwrap_or_default()
    }

    /// Get all layers that have elements.
    #[allow(dead_code)]
    pub fn layers(&self) -> impl Iterator<Item = &Layer> {
        self.layers.keys()
    }
}

/// Build a board overlay for review mode.
///
/// Shows the played move as last-move tints and the engine's best move as outlined squares.
pub fn build_review_overlay(review: &ReviewState) -> BoardOverlay {
    let mut overlay = BoardOverlay::new();

    // Layer 1: Played move highlights (from/to of the actual move played)
    if let Some((from, to)) = review.played_move_squares() {
        overlay.tint(from, OverlayColor::LastMove);
        overlay.tint(to, OverlayColor::LastMove);
    }

    // Layer 2: Best move (engine recommendation) - arrow and outline squares
    if let Some((from, to)) = review.best_move_squares() {
        overlay.arrow(from, to, OverlayColor::BestMove);
        overlay.outline(from, OverlayColor::BestMove);
        overlay.outline(to, OverlayColor::BestMove);
    }

    // Layer 3: Tactical patterns from advanced analysis (new pipeline)
    if let Some(adv_pos) = review.advanced_position() {
        if !adv_pos.tactical_tags_after.is_empty() {
            add_tactical_tag_overlays(&mut overlay, &adv_pos.tactical_tags_after);
        }
    }

    overlay
}

/// Parse a square string like "e4" into a cozy_chess Square.
fn parse_square_str(sq_str: &str) -> Option<Square> {
    if sq_str.len() < 2 {
        return None;
    }
    let bytes = sq_str.as_bytes();
    let file = match bytes[0] {
        b'a'..=b'h' => cozy_chess::File::index((bytes[0] - b'a') as usize),
        _ => return None,
    };
    let rank = match bytes[1] {
        b'1'..=b'8' => cozy_chess::Rank::index((bytes[1] - b'1') as usize),
        _ => return None,
    };
    Some(Square::new(file, rank))
}

/// Add tactical tag overlays to the board using the new TacticalTagProto model.
fn add_tactical_tag_overlays(overlay: &mut BoardOverlay, tags: &[chess_client::TacticalTagProto]) {
    use chess_client::TacticalTagKindProto;

    for tag in tags {
        let kind = TacticalTagKindProto::try_from(tag.kind).ok();

        // Determine overlay color based on tag kind
        let color = match kind {
            Some(TacticalTagKindProto::TacticalTagKindHangingPiece) => OverlayColor::Blunder,
            Some(TacticalTagKindProto::TacticalTagKindBackRankWeakness) => OverlayColor::Danger,
            Some(TacticalTagKindProto::TacticalTagKindMateThreat) => OverlayColor::Danger,
            Some(TacticalTagKindProto::TacticalTagKindPin) => OverlayColor::Danger,
            Some(TacticalTagKindProto::TacticalTagKindSkewer) => OverlayColor::Danger,
            _ => OverlayColor::Tactical,
        };

        // Draw arrows from attacker to each victim
        if let Some(ref attacker_str) = tag.attacker {
            if let Some(from) = parse_square_str(attacker_str) {
                for victim_str in &tag.victims {
                    if let Some(to) = parse_square_str(victim_str) {
                        overlay.arrow(from, to, color);
                    }
                }
                // If no victims but has target_square, draw arrow to target
                if tag.victims.is_empty() {
                    if let Some(ref target_str) = tag.target_square {
                        if let Some(to) = parse_square_str(target_str) {
                            overlay.arrow(from, to, color);
                        }
                    }
                }
            }
        }

        // For hanging pieces and back rank weakness, tint the target square
        match kind {
            Some(TacticalTagKindProto::TacticalTagKindHangingPiece) => {
                if let Some(ref target_str) = tag.target_square {
                    if let Some(sq) = parse_square_str(target_str) {
                        overlay.tint(sq, OverlayColor::Blunder);
                    }
                }
                // Also tint victims as hanging squares
                for victim_str in &tag.victims {
                    if let Some(sq) = parse_square_str(victim_str) {
                        overlay.tint(sq, OverlayColor::Blunder);
                    }
                }
            }
            Some(TacticalTagKindProto::TacticalTagKindBackRankWeakness) => {
                if let Some(ref target_str) = tag.target_square {
                    if let Some(sq) = parse_square_str(target_str) {
                        overlay.tint(sq, OverlayColor::Danger);
                    }
                }
            }
            _ => {}
        }
    }
}

/// Compute an arrow-head symbol based on the direction from `from` to `to`.
#[allow(dead_code)]
fn arrow_symbol(from: Square, to: Square) -> &'static str {
    let df = to.file() as i8 - from.file() as i8;
    let dr = to.rank() as i8 - from.rank() as i8;

    // Normalize to direction
    let df = df.signum();
    let dr = dr.signum();

    match (df, dr) {
        (0, 1) => "↑",
        (0, -1) => "↓",
        (1, 0) => "→",
        (-1, 0) => "←",
        (1, 1) => "↗",
        (-1, 1) => "↖",
        (1, -1) => "↘",
        (-1, -1) => "↙",
        _ => "•",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cozy_chess::{File, Rank};

    fn sq(file: File, rank: Rank) -> Square {
        Square::new(file, rank)
    }

    #[test]
    fn test_overlay_tint_last_wins() {
        let mut overlay = BoardOverlay::new();
        let e4 = sq(File::E, Rank::Fourth);
        overlay.tint(e4, OverlayColor::Selected);
        overlay.tint(e4, OverlayColor::BestMove);

        assert_eq!(overlay.square_tint(e4), Some(OverlayColor::BestMove));
    }

    #[test]
    fn test_overlay_no_tint() {
        let overlay = BoardOverlay::new();
        let e4 = sq(File::E, Rank::Fourth);
        assert_eq!(overlay.square_tint(e4), None);
    }

    #[test]
    fn test_arrow_tints_both_squares() {
        let mut overlay = BoardOverlay::new();
        let e2 = sq(File::E, Rank::Second);
        let e4 = sq(File::E, Rank::Fourth);
        overlay.arrow(e2, e4, OverlayColor::BestMove);

        assert_eq!(overlay.square_tint(e2), Some(OverlayColor::BestMove));
        assert_eq!(overlay.square_tint(e4), Some(OverlayColor::BestMove));
    }

    #[test]
    fn test_arrow_annotation_on_destination() {
        let mut overlay = BoardOverlay::new();
        let e2 = sq(File::E, Rank::Second);
        let e4 = sq(File::E, Rank::Fourth);
        overlay.arrow(e2, e4, OverlayColor::BestMove);

        let ann = overlay.arrow_annotation(e4);
        assert!(ann.is_some());
        let (symbol, color) = ann.unwrap();
        assert_eq!(symbol, "↑");
        assert_eq!(color, OverlayColor::BestMove);

        // No annotation on the from square
        assert!(overlay.arrow_annotation(e2).is_none());
    }

    #[test]
    fn test_arrow_symbol_directions() {
        let e4 = sq(File::E, Rank::Fourth);

        // Up
        assert_eq!(arrow_symbol(e4, sq(File::E, Rank::Eighth)), "↑");
        // Down
        assert_eq!(arrow_symbol(e4, sq(File::E, Rank::First)), "↓");
        // Right
        assert_eq!(arrow_symbol(e4, sq(File::H, Rank::Fourth)), "→");
        // Left
        assert_eq!(arrow_symbol(e4, sq(File::A, Rank::Fourth)), "←");
        // Diagonal
        assert_eq!(arrow_symbol(e4, sq(File::G, Rank::Sixth)), "↗");
        assert_eq!(arrow_symbol(e4, sq(File::C, Rank::Sixth)), "↖");
        assert_eq!(arrow_symbol(e4, sq(File::G, Rank::Second)), "↘");
        assert_eq!(arrow_symbol(e4, sq(File::C, Rank::Second)), "↙");
    }

    #[test]
    fn test_overlay_color_resolve() {
        assert_eq!(OverlayColor::Selected.resolve(true), Color::LightYellow);
        assert_eq!(OverlayColor::Selected.resolve(false), Color::Yellow);
        assert_eq!(OverlayColor::BestMove.resolve(true), Color::LightGreen);
        assert_eq!(OverlayColor::BestMove.resolve(false), Color::Green);
    }

    #[test]
    fn test_outline() {
        let mut overlay = BoardOverlay::new();
        let e4 = sq(File::E, Rank::Fourth);
        let d4 = sq(File::D, Rank::Fourth);

        overlay.outline(e4, OverlayColor::Selected);
        assert_eq!(overlay.square_outline(e4), Some(OverlayColor::Selected));
        assert_eq!(overlay.square_outline(d4), None);
    }
}
