use ratatui::style::Color;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TuiThemeKind {
    Classic,
    Warm,
}

impl TuiThemeKind {
    pub(crate) fn next(self) -> Self {
        match self {
            TuiThemeKind::Classic => TuiThemeKind::Warm,
            TuiThemeKind::Warm => TuiThemeKind::Classic,
        }
    }

    pub(crate) fn name(self) -> &'static str {
        match self {
            TuiThemeKind::Classic => "classic",
            TuiThemeKind::Warm => "warm",
        }
    }

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
            TuiThemeKind::Warm => TuiTheme {
                text: Color::Rgb(245, 245, 244),
                muted: Color::Rgb(161, 161, 170),
                focused_border: Color::Rgb(249, 115, 22),
                border: Color::Rgb(113, 113, 122),
                selected_fg: Color::Black,
                selected_bg: Color::Rgb(249, 115, 22),
                label: Color::Rgb(249, 115, 22),
                error: Color::Rgb(239, 68, 68),
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
