use ratatui::style::Color;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TuiThemeKind {
    Classic,
}

impl TuiThemeKind {
    pub(crate) fn theme(self) -> TuiTheme {
        match self {
            TuiThemeKind::Classic => TuiTheme {
                text: Color::White,
                muted: Color::Gray,
                focused_border: Color::LightCyan,
                border: Color::Gray,
                selected_fg: Color::Black,
                selected_bg: Color::LightGreen,
                label: Color::LightBlue,
                error: Color::Red,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct TuiTheme {
    pub(crate) text: Color,
    pub(crate) muted: Color,
    pub(crate) focused_border: Color,
    pub(crate) border: Color,
    pub(crate) selected_fg: Color,
    pub(crate) selected_bg: Color,
    pub(crate) label: Color,
    pub(crate) error: Color,
}
