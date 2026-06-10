use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, BorderType, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap,
};
use ratatui::{Frame, layout::Alignment};

use crate::config::{CurrentProfile, Target, mask_api_key};

use super::layout::{centered_rect, form_fields, form_layout, tui_layout};
use super::state::{FocusPane, ProfileForm, TuiApp, TuiMode, is_builtin_profile};
use super::theme::TuiTheme;

pub(crate) fn draw_tui(frame: &mut Frame<'_>, app: &TuiApp) {
    let theme = app.theme();
    let layout = tui_layout(frame.area());

    draw_targets(frame, app, layout.targets, theme);
    draw_profiles(frame, app, layout.profiles, theme);
    draw_profile_details(frame, app, layout.details, theme);
    draw_status(frame, app, layout.status, theme);

    match &app.mode {
        TuiMode::Normal => {}
        TuiMode::Editing(form) => draw_form(frame, form, theme),
        TuiMode::ConfirmDelete => draw_delete_confirmation(frame, app, theme),
    }
}

fn draw_targets(frame: &mut Frame<'_>, app: &TuiApp, area: Rect, theme: TuiTheme) {
    let items: Vec<ListItem> = Target::all()
        .into_iter()
        .map(|target| {
            let label = match target {
                Target::Codex => "codex",
                Target::Claude => "claude code",
            };
            ListItem::new(label)
        })
        .collect();
    let mut state = ListState::default();
    state.select(Some(app.target_index));
    let block = pane_block("[1] Tools", app.focus == FocusPane::Targets, theme);
    let list = List::new(items)
        .block(block)
        .highlight_style(selected_style(theme))
        .highlight_symbol("");
    frame.render_stateful_widget(list, area, &mut state);
}

fn draw_profiles(frame: &mut Frame<'_>, app: &TuiApp, area: Rect, theme: TuiTheme) {
    let current = app
        .config
        .current_for_target(Target::all()[app.target_index]);
    let items: Vec<ListItem> = app
        .selected_profile_indices()
        .into_iter()
        .map(|index| {
            let profile = &app.config.profiles[index];
            let marker = if current.is_some_and(|current| current.name == profile.name) {
                "*"
            } else {
                " "
            };
            ListItem::new(format!("{marker} {}", profile.name))
        })
        .collect();
    let mut state = ListState::default();
    if !items.is_empty() {
        state.select(Some(app.profile_index.min(items.len() - 1)));
    }
    let block = pane_block("[2] Profiles", app.focus == FocusPane::Profiles, theme);
    let list = List::new(items)
        .block(block)
        .highlight_style(selected_style(theme))
        .highlight_symbol("");
    frame.render_stateful_widget(list, area, &mut state);
}

fn draw_profile_details(frame: &mut Frame<'_>, app: &TuiApp, area: Rect, theme: TuiTheme) {
    let lines = match app.selected_profile() {
        Some(profile) => {
            let current = app.config.is_current(&CurrentProfile {
                target: profile.target,
                name: profile.name.clone(),
            });
            vec![
                Line::from(vec![
                    Span::styled("Name: ", label_style(theme)),
                    Span::raw(&profile.name),
                ]),
                Line::from(vec![
                    Span::styled("Target: ", label_style(theme)),
                    Span::raw(profile.target.display_name()),
                ]),
                Line::from(vec![
                    Span::styled("Current: ", label_style(theme)),
                    Span::raw(if current { "yes" } else { "no" }),
                ]),
                Line::from(vec![
                    Span::styled("Model: ", label_style(theme)),
                    Span::raw(&profile.model),
                ]),
                Line::from(vec![
                    Span::styled("Base URL: ", label_style(theme)),
                    Span::raw(&profile.base_url),
                ]),
                Line::from(vec![
                    Span::styled("API key: ", label_style(theme)),
                    Span::raw(mask_api_key(&profile.api_key)),
                ]),
                Line::from(vec![
                    Span::styled("Mode: ", label_style(theme)),
                    Span::raw(if is_builtin_profile(profile) {
                        "read-only built-in"
                    } else {
                        "custom"
                    }),
                ]),
            ]
        }
        None => vec![Line::from("No profile selected")],
    };
    let paragraph = Paragraph::new(lines)
        .block(pane_block(
            "[0] Configuration",
            app.focus == FocusPane::Details,
            theme,
        ))
        .wrap(Wrap { trim: false });
    frame.render_widget(paragraph, area);
}

fn draw_status(frame: &mut Frame<'_>, app: &TuiApp, area: Rect, theme: TuiTheme) {
    let status = Paragraph::new(app.message.as_str()).style(Style::default().fg(theme.muted));
    frame.render_widget(status, area);
}

fn draw_form(frame: &mut Frame<'_>, form: &ProfileForm, theme: TuiTheme) {
    let fields = form_fields(form);
    let (area, rows) = form_layout(frame.area(), form);
    frame.render_widget(Clear, area);
    let title = if form.original.is_some() {
        "Edit provider"
    } else {
        "New provider"
    };
    frame.render_widget(pane_block(title, true, theme), area);

    for (index, (label, value)) in fields.iter().enumerate() {
        let shown = if *label == "API key" {
            mask_api_key(value)
        } else {
            (*value).to_string()
        };
        let input = Paragraph::new(shown)
            .block(pane_block(label, index == form.active_field, theme))
            .style(Style::default().fg(theme.text));
        frame.render_widget(input, rows[index + 1]);
    }
    let active_value = match form.active_field {
        0 => form.name.as_str(),
        1 => form.base_url.as_str(),
        2 => form.api_key.as_str(),
        _ => form.model.as_str(),
    };
    let active_area = rows[form.active_field + 1];
    let cursor_offset = active_value
        .chars()
        .count()
        .min(active_area.width.saturating_sub(2) as usize) as u16;
    frame.set_cursor_position((active_area.x + 1 + cursor_offset, active_area.y + 1));

    if let Some(error) = &form.error {
        frame.render_widget(
            Paragraph::new(error.as_str()).style(Style::default().fg(theme.error)),
            rows[fields.len() + 1],
        );
    }
}

fn draw_delete_confirmation(frame: &mut Frame<'_>, app: &TuiApp, theme: TuiTheme) {
    let area = centered_rect(48, 7, frame.area());
    frame.render_widget(Clear, area);
    let name = app
        .selected_profile()
        .map(|profile| profile.name.as_str())
        .unwrap_or("");
    let paragraph = Paragraph::new(vec![
        Line::from(format!("Delete profile '{name}'?")),
        Line::from("Enter/y confirms, Esc/n cancels"),
    ])
    .alignment(Alignment::Center)
    .block(pane_block("Confirm delete", true, theme));
    frame.render_widget(paragraph, area);
}

fn pane_block(title: &'static str, focused: bool, theme: TuiTheme) -> Block<'static> {
    let style = if focused {
        Style::default().fg(theme.focused_border)
    } else {
        Style::default().fg(theme.border)
    };
    Block::default()
        .title(Line::styled(
            title,
            Style::default().add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .style(style)
}

fn selected_style(theme: TuiTheme) -> Style {
    Style::default()
        .fg(theme.selected_fg)
        .bg(theme.selected_bg)
        .add_modifier(Modifier::BOLD)
}

fn label_style(theme: TuiTheme) -> Style {
    Style::default()
        .fg(theme.label)
        .add_modifier(Modifier::BOLD)
}
