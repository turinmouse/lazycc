use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, BorderType, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap,
};
use ratatui::{
    Frame,
    layout::{Alignment, Rect},
};

use crate::config::{CurrentProfile, Target, mask_api_key};
use crate::tools::tool_for;

use super::layout::{centered_rect, form_fields, form_layout, tui_layout};
use super::state::{FocusPane, McpRefreshState, NavigationTab, ProfileForm, TuiApp, TuiMode};
use super::theme::TuiTheme;

pub(crate) fn draw_tui(frame: &mut Frame<'_>, app: &TuiApp) {
    let theme = app.theme();
    let layout = tui_layout(frame.area());

    draw_navigation(frame, app, layout.navigation, theme);
    draw_mcp_servers(frame, app, layout.mcp, theme);
    draw_plugins(frame, app, layout.plugins, theme);
    if app.focus == FocusPane::PluginSearch {
        draw_plugin_search(frame, app, layout.details, theme);
    } else {
        draw_profile_details(frame, app, layout.details, theme);
    }
    draw_status(frame, app, layout.status, theme);

    match &app.mode {
        TuiMode::Normal => {}
        TuiMode::Editing(form) => draw_form(frame, form, theme),
        TuiMode::ConfirmDelete => draw_delete_confirmation(frame, app, theme),
        TuiMode::ConfirmDeleteMcp => draw_delete_mcp_confirmation(frame, app, theme),
        TuiMode::ConfirmDeletePlugin => draw_delete_plugin_confirmation(frame, app, theme),
        TuiMode::ConfirmUninstallPlugin(plugin) => {
            draw_uninstall_plugin_confirmation(frame, plugin, theme)
        }
    }
}

fn draw_navigation(frame: &mut Frame<'_>, app: &TuiApp, area: Rect, theme: TuiTheme) {
    let block = pane_block(
        navigation_title(app.navigation_tab, theme),
        !matches!(app.focus, FocusPane::PluginSearch | FocusPane::Details),
        theme,
    );
    let inner = block.inner(area);
    frame.render_widget(block, area);

    match app.navigation_tab {
        NavigationTab::Targets => draw_targets(frame, app, inner, theme),
        NavigationTab::Profiles => draw_profiles(frame, app, inner, theme),
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
            ListItem::new(format!("  {label}"))
        })
        .collect();
    let mut state = ListState::default();
    state.select(Some(app.target_index));
    let list = List::new(items)
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
    let list = List::new(items)
        .highlight_style(selected_style(theme))
        .highlight_symbol("");
    frame.render_stateful_widget(list, area, &mut state);
}

fn draw_mcp_servers(frame: &mut Frame<'_>, app: &TuiApp, area: Rect, theme: TuiTheme) {
    let block = pane_block(
        Line::styled("[2] MCP", Style::default().add_modifier(Modifier::BOLD)),
        app.focus == FocusPane::Mcp,
        theme,
    );
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let selected_indices = app.selected_mcp_indices();
    let items: Vec<ListItem> = if selected_indices.is_empty() {
        let message = match app.selected_mcp_refresh_state() {
            McpRefreshState::NotLoaded | McpRefreshState::Loading => "  Loading...",
            McpRefreshState::Loaded => "  No MCP servers",
        };
        vec![ListItem::new(message)]
    } else {
        selected_indices
            .into_iter()
            .map(|index| {
                let server = &app.mcp_servers[index];
                ListItem::new(format!("  {}", server.name))
            })
            .collect()
    };
    let mut state = ListState::default();
    if !items.is_empty() && !app.selected_mcp_indices().is_empty() {
        state.select(Some(app.mcp_index.min(items.len() - 1)));
    }
    let list = List::new(items)
        .highlight_style(selected_style(theme))
        .highlight_symbol("");
    frame.render_stateful_widget(list, inner, &mut state);
}

fn draw_plugins(frame: &mut Frame<'_>, app: &TuiApp, area: Rect, theme: TuiTheme) {
    let block = pane_block(
        Line::styled("[3] Plugins", Style::default().add_modifier(Modifier::BOLD)),
        app.focus == FocusPane::Plugins,
        theme,
    );
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let items: Vec<ListItem> = if app.plugins.is_empty() {
        vec![ListItem::new("  No plugins")]
    } else {
        app.plugins
            .iter()
            .map(|plugin| {
                let state = if plugin.enabled {
                    "enabled"
                } else {
                    "disabled"
                };
                ListItem::new(format!("  {:<24} {}", plugin.name, state))
            })
            .collect()
    };
    let mut state = ListState::default();
    if !app.plugins.is_empty() {
        state.select(Some(app.plugin_index.min(items.len() - 1)));
    }
    let list = List::new(items)
        .highlight_style(selected_style(theme))
        .highlight_symbol("");
    frame.render_stateful_widget(list, inner, &mut state);
}

fn draw_plugin_search(frame: &mut Frame<'_>, app: &TuiApp, area: Rect, theme: TuiTheme) {
    let block = pane_block(
        Line::styled(
            "[0] Plugin Search",
            Style::default().add_modifier(Modifier::BOLD),
        ),
        app.focus == FocusPane::PluginSearch,
        theme,
    );
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = ratatui::layout::Layout::vertical([
        ratatui::layout::Constraint::Length(1),
        ratatui::layout::Constraint::Min(1),
        ratatui::layout::Constraint::Length(4),
    ])
    .split(inner);

    let query = if app.plugin_search_query.is_empty() {
        "Search: ".to_string()
    } else {
        format!("Search: {}", app.plugin_search_query)
    };
    frame.render_widget(
        Paragraph::new(query).style(Style::default().fg(theme.label)),
        chunks[0],
    );

    let indices = app.filtered_available_plugin_indices();
    let items: Vec<ListItem> = if indices.is_empty() {
        if app.plugin_search_loading() {
            vec![ListItem::new("  Loading plugins...")]
        } else {
            vec![ListItem::new("  No matching plugins")]
        }
    } else {
        indices
            .iter()
            .filter_map(|index| app.available_plugins.get(*index))
            .map(|plugin| {
                let state = if plugin.installed {
                    if plugin.enabled {
                        "installed"
                    } else {
                        "disabled"
                    }
                } else {
                    "available"
                };
                let marketplace = plugin.marketplace.as_deref().unwrap_or("-");
                ListItem::new(format!(
                    "  {:<8} {:<28} {:<16} {}",
                    plugin.target, plugin.name, marketplace, state
                ))
            })
            .collect()
    };
    let mut state = ListState::default();
    if !indices.is_empty() {
        state.select(Some(app.plugin_search_index.min(indices.len() - 1)));
    }
    let list = List::new(items)
        .highlight_style(selected_style(theme))
        .highlight_symbol("");
    frame.render_stateful_widget(list, chunks[1], &mut state);

    let mut details = match app.selected_available_plugin() {
        Some(plugin) => vec![
            Line::from(vec![
                Span::styled("Selector: ", label_style(theme)),
                Span::raw(&plugin.selector),
            ]),
            Line::from(vec![
                Span::styled("Action: ", label_style(theme)),
                Span::raw(if plugin.installed {
                    "space/enter uninstalls after confirmation"
                } else {
                    "space/enter installs"
                }),
            ]),
            Line::from(vec![
                Span::styled("Details: ", label_style(theme)),
                Span::raw(&plugin.details),
            ]),
        ],
        None => vec![Line::from("No plugin selected")],
    };
    if !app.plugin_search_errors.is_empty() {
        details.push(Line::from(vec![
            Span::styled("Errors: ", label_style(theme)),
            Span::raw(app.plugin_search_errors.join(" | ")),
        ]));
    }
    frame.render_widget(
        Paragraph::new(details).wrap(Wrap { trim: false }),
        chunks[2],
    );
}

fn draw_profile_details(frame: &mut Frame<'_>, app: &TuiApp, area: Rect, theme: TuiTheme) {
    let lines = if app.focus == FocusPane::Mcp {
        match app.selected_mcp() {
            Some(server) => vec![
                Line::from(vec![
                    Span::styled("Name: ", label_style(theme)),
                    Span::raw(&server.name),
                ]),
                Line::from(vec![
                    Span::styled("Target: ", label_style(theme)),
                    Span::raw(tool_for(server.target).display_name()),
                ]),
                Line::from(vec![
                    Span::styled("Details: ", label_style(theme)),
                    Span::raw(&server.details),
                ]),
            ],
            None => vec![Line::from("No MCP server selected")],
        }
    } else if app.focus == FocusPane::Plugins {
        match app.selected_plugin() {
            Some(plugin) => vec![
                Line::from(vec![
                    Span::styled("Name: ", label_style(theme)),
                    Span::raw(&plugin.name),
                ]),
                Line::from(vec![
                    Span::styled("Target: ", label_style(theme)),
                    Span::raw(tool_for(plugin.target).display_name()),
                ]),
                Line::from(vec![
                    Span::styled("Status: ", label_style(theme)),
                    Span::raw(if plugin.enabled {
                        "enabled"
                    } else {
                        "disabled"
                    }),
                ]),
                Line::from(vec![
                    Span::styled("Details: ", label_style(theme)),
                    Span::raw(&plugin.details),
                ]),
            ],
            None => vec![Line::from("No plugin selected")],
        }
    } else {
        match app.selected_profile() {
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
                        Span::raw(tool_for(profile.target).display_name()),
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
                ]
            }
            None => vec![Line::from("No profile selected")],
        }
    };
    let paragraph = Paragraph::new(lines)
        .block(pane_block(
            Line::styled(
                "[0] Configuration",
                Style::default().add_modifier(Modifier::BOLD),
            ),
            app.focus == FocusPane::Details,
            theme,
        ))
        .wrap(Wrap { trim: false });
    frame.render_widget(paragraph, area);
}

fn draw_status(frame: &mut Frame<'_>, app: &TuiApp, area: Rect, theme: TuiTheme) {
    let keybindings = app.keybindings();
    let status = Paragraph::new(Line::from(vec![
        Span::styled(app.message.as_str(), Style::default().fg(theme.label)),
        Span::styled("  |  ", Style::default().fg(theme.muted)),
        Span::styled(keybindings, Style::default().fg(theme.selected_bg)),
    ]));
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
    frame.render_widget(
        pane_block(
            Line::styled(title, Style::default().add_modifier(Modifier::BOLD)),
            true,
            theme,
        ),
        area,
    );

    for (index, (label, value)) in fields.iter().enumerate() {
        let shown = if *label == "API key" {
            mask_api_key(value)
        } else {
            (*value).to_string()
        };
        let input = Paragraph::new(shown)
            .block(pane_block(
                Line::styled(*label, Style::default().add_modifier(Modifier::BOLD)),
                index == form.active_field,
                theme,
            ))
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
    .block(pane_block(
        Line::styled(
            "Confirm delete",
            Style::default().add_modifier(Modifier::BOLD),
        ),
        true,
        theme,
    ));
    frame.render_widget(paragraph, area);
}

fn draw_delete_mcp_confirmation(frame: &mut Frame<'_>, app: &TuiApp, theme: TuiTheme) {
    let area = centered_rect(52, 7, frame.area());
    frame.render_widget(Clear, area);
    let name = app
        .selected_mcp()
        .map(|server| server.name.as_str())
        .unwrap_or("");
    let paragraph = Paragraph::new(vec![
        Line::from(format!("Delete MCP server '{name}'?")),
        Line::from("Enter/y confirms, Esc/n cancels"),
    ])
    .alignment(Alignment::Center)
    .block(pane_block(
        Line::styled(
            "Confirm delete MCP",
            Style::default().add_modifier(Modifier::BOLD),
        ),
        true,
        theme,
    ));
    frame.render_widget(paragraph, area);
}

fn draw_delete_plugin_confirmation(frame: &mut Frame<'_>, app: &TuiApp, theme: TuiTheme) {
    let area = centered_rect(52, 7, frame.area());
    frame.render_widget(Clear, area);
    let name = app
        .selected_plugin()
        .map(|plugin| plugin.name.as_str())
        .unwrap_or("selected plugin");
    let paragraph = Paragraph::new(vec![
        Line::from("Delete plugin?"),
        Line::from(name.to_string()),
        Line::from("Enter/y confirms, Esc/n cancels"),
    ])
    .alignment(Alignment::Center)
    .block(pane_block(
        Line::styled("Confirm", Style::default().add_modifier(Modifier::BOLD)),
        true,
        theme,
    ));
    frame.render_widget(paragraph, area);
}

fn draw_uninstall_plugin_confirmation(
    frame: &mut Frame<'_>,
    plugin: &crate::tools::Plugin,
    theme: TuiTheme,
) {
    let area = centered_rect(52, 7, frame.area());
    frame.render_widget(Clear, area);
    let paragraph = Paragraph::new(vec![
        Line::from("Uninstall plugin?"),
        Line::from(format!("{} ({})", plugin.name, plugin.target)),
        Line::from("Enter/y confirms, Esc/n cancels"),
    ])
    .alignment(Alignment::Center)
    .block(pane_block(
        Line::styled(
            "Confirm uninstall",
            Style::default().add_modifier(Modifier::BOLD),
        ),
        true,
        theme,
    ));
    frame.render_widget(paragraph, area);
}

fn pane_block(title: Line<'static>, focused: bool, theme: TuiTheme) -> Block<'static> {
    let style = if focused {
        Style::default().fg(theme.focused_border)
    } else {
        Style::default().fg(theme.border)
    };
    Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .style(style)
}

fn navigation_title(tab: NavigationTab, theme: TuiTheme) -> Line<'static> {
    let selected = label_style(theme);
    let muted = Style::default().fg(theme.muted);
    let targets_style = if tab == NavigationTab::Targets {
        selected
    } else {
        muted
    };
    let profiles_style = if tab == NavigationTab::Profiles {
        selected
    } else {
        muted
    };

    Line::from(vec![
        Span::styled("[1]", Style::default().add_modifier(Modifier::BOLD)),
        Span::styled(" - ", muted),
        Span::styled("Targets", targets_style),
        Span::styled(" - ", muted),
        Span::styled("Profiles", profiles_style),
    ])
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
