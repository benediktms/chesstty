use serde::{Deserialize, Serialize};

pub use super::component::Component;

use cozy_chess::Square;

// ============================================================================
// Input Phase - tracks move input state
// ============================================================================

#[derive(Clone, Debug, Copy, PartialEq, Default)]
pub enum InputPhase {
    #[default]
    SelectPiece,
    SelectDestination,
    SelectPromotion {
        from: Square,
        to: Square,
    },
}

// ============================================================================
// Tab Input State - for tab-based move input with typeahead
// ============================================================================

#[derive(Clone, Debug)]
pub struct TabInputState {
    pub active: bool,
    pub current_tab: usize,
    pub typeahead_buffer: String,
    pub from_square: Option<Square>,
}

impl Default for TabInputState {
    fn default() -> Self {
        Self::new()
    }
}

impl TabInputState {
    pub fn new() -> Self {
        Self {
            active: false,
            current_tab: 0,
            typeahead_buffer: String::new(),
            from_square: None,
        }
    }

    pub fn activate(&mut self) {
        self.active = true;
        self.current_tab = 0;
        self.typeahead_buffer.clear();
        self.from_square = None;
    }

    pub fn deactivate(&mut self) {
        self.active = false;
        self.typeahead_buffer.clear();
        self.from_square = None;
    }

    pub fn advance_to_destination(&mut self, from: Square) {
        self.current_tab = 1;
        self.from_square = Some(from);
        self.typeahead_buffer.clear();
    }
}

/// A control displayed to the user (key + label)
#[derive(Clone, Debug, PartialEq)]
pub struct Control {
    pub key: &'static str,
    pub label: &'static str,
}

impl Control {
    pub fn new(key: &'static str, label: &'static str) -> Self {
        Self { key, label }
    }
}

/// Overlay types - dialogs
/// Note: Dialog state is managed in GameSession, this just tracks what's active
#[derive(Clone, Debug, PartialEq, Default)]
pub enum Overlay {
    #[default]
    None,
    PopupMenu,
    SnapshotDialog,
    PromotionDialog {
        from: Square,
        to: Square,
    },
}

/// Layout constraint types
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Constraint {
    Percentage(u16),
    Min(u16),
    Length(u16),
    Ratio(u16, u16),
}

impl Default for Constraint {
    fn default() -> Self {
        Constraint::Min(10)
    }
}

/// Section content - either a component or nested sections
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum SectionContent {
    Component(Component),
    Nested(Vec<Section>),
}

impl Default for SectionContent {
    fn default() -> Self {
        SectionContent::Component(Component::Board)
    }
}

/// A section in a layout row
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Section {
    pub constraint: Constraint,
    pub content: SectionContent,
    /// When true, the section renders with dimmed chrome and no content.
    /// Used for the sidebar instance of an expanded panel.
    #[serde(default)]
    pub dimmed: bool,
}

impl Section {
    #[allow(dead_code)] // generic constructor, callers use component()/nested() instead
    pub fn new(constraint: Constraint, content: SectionContent) -> Self {
        Self {
            constraint,
            content,
            dimmed: false,
        }
    }

    pub fn component(constraint: Constraint, component: Component) -> Self {
        Self {
            constraint,
            content: SectionContent::Component(component),
            dimmed: false,
        }
    }

    pub fn nested(constraint: Constraint, sections: Vec<Section>) -> Self {
        Self {
            constraint,
            content: SectionContent::Nested(sections),
            dimmed: false,
        }
    }

    /// Mark this section as dimmed (grayed-out chrome, no content).
    pub fn with_dimmed(mut self, dimmed: bool) -> Self {
        self.dimmed = dimmed;
        self
    }
}

/// A row in a layout
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Row {
    pub height: Constraint,
    pub sections: Vec<Section>,
}

impl Row {
    pub fn new(height: Constraint, sections: Vec<Section>) -> Self {
        Self { height, sections }
    }
}

/// The complete layout specification for a view
#[derive(Clone, Debug, Default)]
pub struct Layout {
    pub rows: Vec<Row>,
    pub overlay: Overlay,
}

impl Layout {
    /// Start screen - just the menu, no special layout needed
    pub fn start_screen() -> Self {
        Self::default()
    }

    /// Match summary layout - just controls at bottom
    pub fn match_summary() -> Self {
        Self {
            rows: vec![Row::new(
                Constraint::Length(1),
                vec![Section::component(
                    Constraint::Percentage(100),
                    Component::Controls,
                )],
            )],
            overlay: Overlay::None,
        }
    }
}
