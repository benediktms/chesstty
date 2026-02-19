use ratatui::style::Color;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Component {
    Board,
    TabInput,
    Controls,
    InfoPanel,
    HistoryPanel,
    EnginePanel,
    DebugPanel,
    ReviewTabs,
    ReviewSummary,
    AdvancedAnalysis,
}

pub struct ComponentProperties {
    pub component: Component,
    pub title: &'static str,
    pub is_selectable: bool,
    pub is_expandable: bool,
    pub border_color: Color,
    pub preferred_height: u16,
}

impl ComponentProperties {
    pub fn for_component(component: &Component) -> ComponentProperties {
        match component {
            Component::Board => ComponentProperties {
                component: Component::Board,
                title: "Board",
                is_selectable: false,
                is_expandable: false,
                border_color: Color::Reset,
                preferred_height: 0,
            },
            Component::TabInput => ComponentProperties {
                component: Component::TabInput,
                title: "Tab Input",
                is_selectable: false,
                is_expandable: false,
                border_color: Color::Reset,
                preferred_height: 3,
            },
            Component::Controls => ComponentProperties {
                component: Component::Controls,
                title: "Controls",
                is_selectable: false,
                is_expandable: false,
                border_color: Color::Reset,
                preferred_height: 1,
            },
            Component::InfoPanel => ComponentProperties {
                component: Component::InfoPanel,
                title: "Game Info",
                is_selectable: true,
                is_expandable: false,
                border_color: Color::Cyan,
                preferred_height: 10,
            },
            Component::HistoryPanel => ComponentProperties {
                component: Component::HistoryPanel,
                title: "Move History",
                is_selectable: true,
                is_expandable: true,
                border_color: Color::Cyan,
                preferred_height: 15,
            },
            Component::EnginePanel => ComponentProperties {
                component: Component::EnginePanel,
                title: "Engine Analysis",
                is_selectable: true,
                is_expandable: true,
                border_color: Color::Cyan,
                preferred_height: 12,
            },
            Component::DebugPanel => ComponentProperties {
                component: Component::DebugPanel,
                title: "UCI Debug",
                is_selectable: true,
                is_expandable: true,
                border_color: Color::Magenta,
                preferred_height: 15,
            },
            Component::ReviewTabs => ComponentProperties {
                component: Component::ReviewTabs,
                title: "Review Tabs",
                is_selectable: false,
                is_expandable: false,
                border_color: Color::Green,
                preferred_height: 3,
            },
            Component::ReviewSummary => ComponentProperties {
                component: Component::ReviewSummary,
                title: "Review Summary",
                is_selectable: true,
                is_expandable: true,
                border_color: Color::Green,
                preferred_height: 15,
            },
            Component::AdvancedAnalysis => ComponentProperties {
                component: Component::AdvancedAnalysis,
                title: "Advanced Analysis",
                is_selectable: true,
                is_expandable: true,
                border_color: Color::Magenta,
                preferred_height: 18,
            },
        }
    }
}

impl Component {
    pub fn properties(&self) -> ComponentProperties {
        ComponentProperties::for_component(self)
    }

    pub fn title(&self) -> &'static str {
        self.properties().title
    }

    pub fn is_selectable(&self) -> bool {
        self.properties().is_selectable
    }

    pub fn is_expandable(&self) -> bool {
        self.properties().is_expandable
    }

    pub fn is_panel(&self) -> bool {
        matches!(
            self,
            Component::InfoPanel
                | Component::HistoryPanel
                | Component::EnginePanel
                | Component::DebugPanel
                | Component::ReviewTabs
                | Component::ReviewSummary
                | Component::AdvancedAnalysis
        )
    }
}
