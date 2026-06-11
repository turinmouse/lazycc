use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

use crate::config::{
    Config, CurrentProfile, DEFAULT_CLAUDE_PROFILE, DEFAULT_CODEX_PROFILE, Profile, Target,
    default_current_profile,
};
use crate::tools::{McpServer, Plugin, tool_for};

use super::layout::{
    FORM_FIELD_COUNT, TuiLayout, centered_rect, form_layout, list_index_in_area,
    navigation_list_area, rect_contains, tui_layout,
};
use super::theme::{TuiTheme, TuiThemeKind};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum McpRefreshState {
    NotLoaded,
    Loading,
    Loaded,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FocusPane {
    Targets,
    Profiles,
    Mcp,
    Plugins,
    Details,
    PluginSearch,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum NavigationTab {
    Targets,
    Profiles,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TuiAction {
    None,
    Save,
    Quit,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum TuiMode {
    Normal,
    Editing(ProfileForm),
    ConfirmDelete,
    ConfirmDeleteMcp,
    ConfirmDeletePlugin,
    ConfirmUninstallPlugin(Plugin),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CommandHint {
    label: &'static str,
    keys: &'static str,
}

impl CommandHint {
    const fn new(label: &'static str, keys: &'static str) -> Self {
        Self { label, keys }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ProfileForm {
    pub(crate) original: Option<CurrentProfile>,
    pub(crate) target: Target,
    pub(crate) active_field: usize,
    pub(crate) name: String,
    pub(crate) base_url: String,
    pub(crate) api_key: String,
    pub(crate) model: String,
    pub(crate) error: Option<String>,
}

impl ProfileForm {
    pub(crate) fn add(target: Target) -> Self {
        Self {
            original: None,
            target,
            active_field: 0,
            name: String::new(),
            base_url: String::new(),
            api_key: String::new(),
            model: String::new(),
            error: None,
        }
    }

    pub(crate) fn edit(profile: &Profile) -> Self {
        Self {
            original: Some(CurrentProfile {
                target: profile.target,
                name: profile.name.clone(),
            }),
            target: profile.target,
            active_field: 0,
            name: profile.name.clone(),
            base_url: profile.base_url.clone(),
            api_key: profile.api_key.clone(),
            model: profile.model.clone(),
            error: None,
        }
    }

    fn field_mut(&mut self) -> &mut String {
        match self.active_field {
            0 => &mut self.name,
            1 => &mut self.base_url,
            2 => &mut self.api_key,
            _ => &mut self.model,
        }
    }

    fn next_field(&mut self) {
        self.active_field = (self.active_field + 1).min(3);
    }

    fn previous_field(&mut self) {
        self.active_field = self.active_field.saturating_sub(1);
    }

    fn profile(&self) -> Profile {
        Profile {
            name: self.name.trim().to_string(),
            target: self.target,
            base_url: self.base_url.trim().to_string(),
            api_key: self.api_key.trim().to_string(),
            model: self.model.trim().to_string(),
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct TuiApp {
    pub(crate) config: Config,
    pub(crate) target_index: usize,
    pub(crate) profile_index: usize,
    pub(crate) focus: FocusPane,
    pub(crate) navigation_tab: NavigationTab,
    pub(crate) mcp_index: usize,
    pub(crate) mcp_servers: Vec<McpServer>,
    pub(crate) mcp_refresh_states: [McpRefreshState; 2],
    pub(crate) mcp_refresh_requests: Vec<Target>,
    pub(crate) plugin_index: usize,
    pub(crate) plugins: Vec<Plugin>,
    pub(crate) plugin_refresh_states: [McpRefreshState; 2],
    pub(crate) plugin_refresh_requests: Vec<Target>,
    pub(crate) plugin_search_index: usize,
    pub(crate) plugin_search_query: String,
    pub(crate) available_plugins: Vec<Plugin>,
    pub(crate) plugin_search_errors: Vec<String>,
    pub(crate) plugin_search_loaded: bool,
    pub(crate) mode: TuiMode,
    pub(crate) theme_kind: TuiThemeKind,
    pub(crate) message: String,
    pub(crate) should_quit: bool,
}

impl TuiApp {
    pub(crate) fn new(config: Config) -> Self {
        let mut app = Self {
            config,
            target_index: 0,
            profile_index: 0,
            focus: FocusPane::Targets,
            navigation_tab: NavigationTab::Targets,
            mcp_index: 0,
            mcp_servers: Vec::new(),
            mcp_refresh_states: [McpRefreshState::NotLoaded; 2],
            mcp_refresh_requests: Vec::new(),
            plugin_index: 0,
            plugins: load_plugins(),
            plugin_refresh_states: [McpRefreshState::NotLoaded; 2],
            plugin_refresh_requests: Vec::new(),
            plugin_search_index: 0,
            plugin_search_query: String::new(),
            available_plugins: Vec::new(),
            plugin_search_errors: Vec::new(),
            plugin_search_loaded: false,
            mode: TuiMode::Normal,
            theme_kind: TuiThemeKind::Classic,
            message: "lazycc".to_string(),
            should_quit: false,
        };
        app.select_current_target();
        app
    }

    pub(crate) fn handle_key(&mut self, key: KeyEvent) -> TuiAction {
        match self.mode.clone() {
            TuiMode::Normal => self.handle_normal_key(key),
            TuiMode::Editing(mut form) => self.handle_form_key(key, &mut form),
            TuiMode::ConfirmDelete => self.handle_delete_key(key),
            TuiMode::ConfirmDeleteMcp => self.handle_delete_mcp_key(key),
            TuiMode::ConfirmDeletePlugin => self.handle_delete_plugin_key(key),
            TuiMode::ConfirmUninstallPlugin(plugin) => {
                self.handle_uninstall_plugin_key(key, plugin)
            }
        }
    }

    pub(crate) fn handle_mouse(&mut self, mouse: MouseEvent, area: Rect) -> TuiAction {
        match self.mode.clone() {
            TuiMode::Normal => self.handle_normal_mouse(mouse, area),
            TuiMode::Editing(mut form) => self.handle_form_mouse(mouse, area, &mut form),
            TuiMode::ConfirmDelete => self.handle_delete_mouse(mouse, area),
            TuiMode::ConfirmDeleteMcp => self.handle_delete_mcp_mouse(mouse, area),
            TuiMode::ConfirmDeletePlugin => self.handle_delete_plugin_mouse(mouse, area),
            TuiMode::ConfirmUninstallPlugin(plugin) => {
                self.handle_uninstall_plugin_mouse(mouse, area, plugin)
            }
        }
    }

    pub(crate) fn request_mcp_refresh(&mut self) -> bool {
        let mut requested = false;
        for target in Target::all() {
            requested = self.request_mcp_refresh_for(target) || requested;
        }
        requested
    }

    pub(crate) fn request_mcp_refresh_for(&mut self, target: Target) -> bool {
        let index = target_refresh_index(target);
        if self.mcp_refresh_requests.contains(&target)
            || self.mcp_refresh_states[index] == McpRefreshState::Loading
        {
            return false;
        }
        self.mcp_refresh_states[index] = McpRefreshState::Loading;
        self.mcp_refresh_requests.push(target);
        self.message = format!("Loading MCP servers for {target}...");
        true
    }

    pub(crate) fn take_mcp_refresh_requests(&mut self) -> Vec<Target> {
        std::mem::take(&mut self.mcp_refresh_requests)
    }

    pub(crate) fn finish_mcp_refresh(
        &mut self,
        target: Target,
        mut servers: Vec<McpServer>,
        error: Option<String>,
    ) {
        self.mcp_servers.retain(|server| server.target != target);
        self.mcp_servers.append(&mut servers);
        self.mcp_index = self
            .mcp_index
            .min(self.selected_mcp_indices().len().saturating_sub(1));
        self.mcp_refresh_states[target_refresh_index(target)] = McpRefreshState::Loaded;
        if target == self.selected_target() || self.focus == FocusPane::Mcp {
            self.message = error.unwrap_or_else(|| format!("Loaded MCP servers for {target}"));
        }
    }

    pub(crate) fn selected_mcp_refresh_state(&self) -> McpRefreshState {
        self.mcp_refresh_state_for(self.selected_target())
    }

    pub(crate) fn mcp_refresh_state_for(&self, target: Target) -> McpRefreshState {
        self.mcp_refresh_states[target_refresh_index(target)]
    }

    pub(crate) fn request_plugin_refresh(&mut self) -> bool {
        let mut requested = false;
        for target in Target::all() {
            requested = self.request_plugin_refresh_for(target) || requested;
        }
        requested
    }

    pub(crate) fn request_plugin_refresh_for(&mut self, target: Target) -> bool {
        let index = target_refresh_index(target);
        if self.plugin_refresh_requests.contains(&target)
            || self.plugin_refresh_states[index] == McpRefreshState::Loading
        {
            return false;
        }
        self.plugin_refresh_states[index] = McpRefreshState::Loading;
        self.plugin_refresh_requests.push(target);
        true
    }

    pub(crate) fn take_plugin_refresh_requests(&mut self) -> Vec<Target> {
        std::mem::take(&mut self.plugin_refresh_requests)
    }

    pub(crate) fn finish_plugin_refresh(
        &mut self,
        target: Target,
        mut plugins: Vec<Plugin>,
        error: Option<String>,
    ) {
        self.available_plugins
            .retain(|plugin| plugin.target != target);
        self.available_plugins.append(&mut plugins);
        self.available_plugins.sort_by(|left, right| {
            left.target
                .to_string()
                .cmp(&right.target.to_string())
                .then_with(|| left.name.cmp(&right.name))
                .then_with(|| left.selector.cmp(&right.selector))
        });

        let label = tool_for(target).display_name().to_string();
        self.plugin_search_errors
            .retain(|message| !message.starts_with(&format!("{label}: ")));
        if let Some(error) = error {
            self.plugin_search_errors.push(format!("{label}: {error}"));
        }
        self.plugin_refresh_states[target_refresh_index(target)] = McpRefreshState::Loaded;
        self.plugin_search_loaded = self
            .plugin_refresh_states
            .iter()
            .all(|state| *state == McpRefreshState::Loaded);
        self.plugin_search_index = self.plugin_search_index.min(
            self.filtered_available_plugin_indices()
                .len()
                .saturating_sub(1),
        );
    }

    pub(crate) fn plugin_search_loading(&self) -> bool {
        self.plugin_refresh_states
            .contains(&McpRefreshState::Loading)
    }

    pub(crate) fn theme(&self) -> TuiTheme {
        self.theme_kind.theme()
    }

    pub(crate) fn keybindings(&self) -> String {
        format_command_hints(&self.command_hints())
    }

    fn command_hints(&self) -> Vec<CommandHint> {
        match &self.mode {
            TuiMode::Normal => match self.focus {
                FocusPane::Targets => vec![
                    CommandHint::new("Open profiles", "<space>/enter"),
                    CommandHint::new("MCP", "2"),
                    CommandHint::new("Details", "0"),
                    CommandHint::new("Theme", "t"),
                    CommandHint::new("Quit", "q"),
                ],
                FocusPane::Profiles => vec![
                    CommandHint::new("Use profile", "<space>/enter"),
                    CommandHint::new("Add", "n"),
                    CommandHint::new("Edit", "e"),
                    CommandHint::new("Delete", "d"),
                    CommandHint::new("Back", "esc"),
                    CommandHint::new("MCP", "2"),
                ],
                FocusPane::Mcp => vec![
                    CommandHint::new("Delete MCP", "d"),
                    CommandHint::new("Plugins", "3"),
                    CommandHint::new("Tools", "1"),
                    CommandHint::new("Details", "0"),
                    CommandHint::new("Back", "esc"),
                    CommandHint::new("Theme", "t"),
                    CommandHint::new("Quit", "q"),
                ],
                FocusPane::Plugins => vec![
                    CommandHint::new("Toggle plugin", "e/space"),
                    CommandHint::new("Delete", "d"),
                    CommandHint::new("MCP", "2"),
                    CommandHint::new("Tools", "1"),
                    CommandHint::new("Search", "n"),
                    CommandHint::new("Details", "0"),
                    CommandHint::new("Back", "esc"),
                ],
                FocusPane::Details => vec![
                    CommandHint::new("Tools", "1"),
                    CommandHint::new("MCP", "2"),
                    CommandHint::new("Plugins", "3"),
                    CommandHint::new("Back", "esc"),
                    CommandHint::new("Theme", "t"),
                    CommandHint::new("Quit", "q"),
                ],
                FocusPane::PluginSearch => vec![
                    CommandHint::new("Install/uninstall", "<space>/enter"),
                    CommandHint::new("Search", "type"),
                    CommandHint::new("Clear", "backspace"),
                    CommandHint::new("Tools", "1"),
                    CommandHint::new("MCP", "2"),
                    CommandHint::new("Plugins", "3"),
                    CommandHint::new("Back", "esc"),
                    CommandHint::new("Theme", "t"),
                    CommandHint::new("Quit", "q"),
                ],
            },
            TuiMode::Editing(_) => vec![
                CommandHint::new("Next field", "tab/down"),
                CommandHint::new("Previous", "shift-tab/up"),
                CommandHint::new("Save", "enter/ctrl-s"),
                CommandHint::new("Cancel", "esc"),
            ],
            TuiMode::ConfirmDelete
            | TuiMode::ConfirmDeleteMcp
            | TuiMode::ConfirmDeletePlugin
            | TuiMode::ConfirmUninstallPlugin(_) => vec![
                CommandHint::new("Confirm", "enter/y"),
                CommandHint::new("Cancel", "esc/n"),
            ],
        }
    }

    fn handle_normal_key(&mut self, key: KeyEvent) -> TuiAction {
        match key.code {
            KeyCode::Char('q') => TuiAction::Quit,
            KeyCode::Esc => self.back_or_quit(),
            KeyCode::Tab | KeyCode::BackTab => TuiAction::None,
            KeyCode::Left => {
                if self.focus != FocusPane::Targets && self.focus != FocusPane::Profiles {
                    self.focus_previous_left_pane();
                }
                TuiAction::None
            }
            KeyCode::Right => {
                if self.focus != FocusPane::Targets && self.focus != FocusPane::Profiles {
                    self.focus_next_left_pane();
                }
                TuiAction::None
            }
            KeyCode::Up => {
                self.move_selection(-1);
                TuiAction::None
            }
            KeyCode::Down => {
                self.move_selection(1);
                TuiAction::None
            }
            KeyCode::Char(value @ ('0'..='5')) => {
                self.select_numbered_pane(value);
                TuiAction::None
            }
            KeyCode::Char('t') => {
                self.toggle_theme();
                TuiAction::None
            }
            KeyCode::Enter | KeyCode::Char(' ') => self.activate_selection(),
            KeyCode::Char('n') | KeyCode::Char('a') => {
                match self.focus {
                    FocusPane::Profiles => {
                        self.mode = TuiMode::Editing(ProfileForm::add(self.selected_target()));
                    }
                    FocusPane::Plugins if matches!(key.code, KeyCode::Char('n')) => {
                        self.set_focus(FocusPane::PluginSearch);
                        self.message = "Search plugins".to_string();
                    }
                    _ => {}
                }
                TuiAction::None
            }
            KeyCode::Backspace => {
                if self.focus == FocusPane::PluginSearch {
                    self.plugin_search_query.pop();
                    self.plugin_search_index = self.plugin_search_index.min(
                        self.filtered_available_plugin_indices()
                            .len()
                            .saturating_sub(1),
                    );
                }
                TuiAction::None
            }
            KeyCode::Char(value)
                if self.focus == FocusPane::PluginSearch && !value.is_control() =>
            {
                self.plugin_search_query.push(value);
                self.plugin_search_index = 0;
                TuiAction::None
            }
            KeyCode::Char('e') => {
                if self.focus == FocusPane::Plugins {
                    self.toggle_selected_plugin();
                } else {
                    self.open_edit_form();
                }
                TuiAction::None
            }
            KeyCode::Char('d') => {
                if self.focus == FocusPane::Mcp {
                    self.open_delete_mcp_confirmation();
                } else if self.focus == FocusPane::Plugins {
                    self.open_delete_plugin_confirmation();
                } else {
                    self.open_delete_confirmation();
                }
                TuiAction::None
            }
            _ => TuiAction::None,
        }
    }

    fn toggle_theme(&mut self) {
        self.theme_kind = self.theme_kind.next();
        self.message = format!("Theme: {}", self.theme_kind.name());
    }

    fn handle_form_key(&mut self, key: KeyEvent, form: &mut ProfileForm) -> TuiAction {
        match key.code {
            KeyCode::Esc => {
                self.mode = TuiMode::Normal;
                TuiAction::None
            }
            KeyCode::Tab | KeyCode::Down => {
                form.next_field();
                self.mode = TuiMode::Editing(form.clone());
                TuiAction::None
            }
            KeyCode::BackTab | KeyCode::Up => {
                form.previous_field();
                self.mode = TuiMode::Editing(form.clone());
                TuiAction::None
            }
            KeyCode::Enter => self.save_form(form),
            KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.save_form(form)
            }
            KeyCode::Char(value) => {
                form.field_mut().push(value);
                form.error = None;
                self.mode = TuiMode::Editing(form.clone());
                TuiAction::None
            }
            KeyCode::Backspace => {
                form.field_mut().pop();
                form.error = None;
                self.mode = TuiMode::Editing(form.clone());
                TuiAction::None
            }
            _ => TuiAction::None,
        }
    }

    fn handle_delete_key(&mut self, key: KeyEvent) -> TuiAction {
        match key.code {
            KeyCode::Char('y') | KeyCode::Enter => self.delete_selected_profile(),
            KeyCode::Char('n') | KeyCode::Esc => {
                self.mode = TuiMode::Normal;
                TuiAction::None
            }
            _ => TuiAction::None,
        }
    }

    fn handle_delete_mcp_key(&mut self, key: KeyEvent) -> TuiAction {
        match key.code {
            KeyCode::Char('y') | KeyCode::Enter => {
                self.delete_selected_mcp();
                TuiAction::None
            }
            KeyCode::Char('n') | KeyCode::Esc => {
                self.mode = TuiMode::Normal;
                TuiAction::None
            }
            _ => TuiAction::None,
        }
    }

    fn handle_delete_plugin_key(&mut self, key: KeyEvent) -> TuiAction {
        match key.code {
            KeyCode::Char('y') | KeyCode::Enter => {
                self.delete_selected_plugin();
                TuiAction::None
            }
            KeyCode::Char('n') | KeyCode::Esc => {
                self.mode = TuiMode::Normal;
                TuiAction::None
            }
            _ => TuiAction::None,
        }
    }

    fn handle_uninstall_plugin_key(&mut self, key: KeyEvent, plugin: Plugin) -> TuiAction {
        match key.code {
            KeyCode::Char('y') | KeyCode::Enter => {
                self.uninstall_plugin(plugin);
                TuiAction::None
            }
            KeyCode::Char('n') | KeyCode::Esc => {
                self.mode = TuiMode::Normal;
                TuiAction::None
            }
            _ => TuiAction::None,
        }
    }

    fn handle_normal_mouse(&mut self, mouse: MouseEvent, area: Rect) -> TuiAction {
        let layout = tui_layout(area);
        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if rect_contains(layout.navigation, mouse.column, mouse.row) {
                    self.handle_navigation_click(layout.navigation, mouse.column, mouse.row);
                } else if rect_contains(layout.mcp, mouse.column, mouse.row) {
                    self.handle_mcp_click(layout.mcp, mouse.column, mouse.row);
                } else if rect_contains(layout.plugins, mouse.column, mouse.row) {
                    self.handle_plugin_click(layout.plugins, mouse.column, mouse.row);
                } else if rect_contains(layout.details, mouse.column, mouse.row) {
                    self.set_focus(FocusPane::Details);
                }
                TuiAction::None
            }
            MouseEventKind::ScrollUp => {
                if self.focus_from_mouse(layout, mouse.column, mouse.row) {
                    self.move_selection(-1);
                }
                TuiAction::None
            }
            MouseEventKind::ScrollDown => {
                if self.focus_from_mouse(layout, mouse.column, mouse.row) {
                    self.move_selection(1);
                }
                TuiAction::None
            }
            _ => TuiAction::None,
        }
    }

    fn handle_form_mouse(
        &mut self,
        mouse: MouseEvent,
        area: Rect,
        form: &mut ProfileForm,
    ) -> TuiAction {
        if !matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left)) {
            return TuiAction::None;
        }

        let (_, rows) = form_layout(area, form);
        for field_index in 0..FORM_FIELD_COUNT {
            if rect_contains(rows[field_index + 1], mouse.column, mouse.row) {
                form.active_field = field_index;
                self.mode = TuiMode::Editing(form.clone());
                return TuiAction::None;
            }
        }
        TuiAction::None
    }

    fn handle_delete_mouse(&mut self, mouse: MouseEvent, area: Rect) -> TuiAction {
        if !matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left)) {
            return TuiAction::None;
        }

        let dialog = centered_rect(48, 7, area);
        if rect_contains(dialog, mouse.column, mouse.row) {
            self.delete_selected_profile()
        } else {
            self.mode = TuiMode::Normal;
            TuiAction::None
        }
    }

    fn handle_delete_mcp_mouse(&mut self, mouse: MouseEvent, area: Rect) -> TuiAction {
        if !matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left)) {
            return TuiAction::None;
        }

        let dialog = centered_rect(52, 7, area);
        if rect_contains(dialog, mouse.column, mouse.row) {
            self.delete_selected_mcp();
        } else {
            self.mode = TuiMode::Normal;
        }
        TuiAction::None
    }

    fn handle_delete_plugin_mouse(&mut self, mouse: MouseEvent, area: Rect) -> TuiAction {
        if !matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left)) {
            return TuiAction::None;
        }

        let dialog = centered_rect(52, 7, area);
        if rect_contains(dialog, mouse.column, mouse.row) {
            self.delete_selected_plugin();
        } else {
            self.mode = TuiMode::Normal;
        }
        TuiAction::None
    }

    fn handle_uninstall_plugin_mouse(
        &mut self,
        mouse: MouseEvent,
        area: Rect,
        plugin: Plugin,
    ) -> TuiAction {
        if !matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left)) {
            return TuiAction::None;
        }

        let dialog = centered_rect(52, 7, area);
        if rect_contains(dialog, mouse.column, mouse.row) {
            self.uninstall_plugin(plugin);
        } else {
            self.mode = TuiMode::Normal;
        }
        TuiAction::None
    }

    fn focus_from_mouse(&mut self, layout: TuiLayout, column: u16, row: u16) -> bool {
        if rect_contains(layout.navigation, column, row) {
            match self.navigation_tab {
                NavigationTab::Targets => self.set_focus(FocusPane::Targets),
                NavigationTab::Profiles => self.set_focus(FocusPane::Profiles),
            }
            true
        } else if rect_contains(layout.mcp, column, row) {
            self.set_focus(FocusPane::Mcp);
            true
        } else if rect_contains(layout.plugins, column, row) {
            self.set_focus(FocusPane::Plugins);
            true
        } else if rect_contains(layout.details, column, row) {
            self.set_focus(FocusPane::Details);
            true
        } else {
            false
        }
    }

    fn handle_mcp_click(&mut self, area: Rect, column: u16, row: u16) {
        let list_area = navigation_list_area(area);
        if let Some(index) =
            list_index_in_area(list_area, column, row, self.selected_mcp_indices().len())
        {
            self.mcp_index = index;
        }
        self.set_focus(FocusPane::Mcp);
    }

    fn handle_plugin_click(&mut self, area: Rect, column: u16, row: u16) {
        let list_area = navigation_list_area(area);
        if let Some(index) = list_index_in_area(list_area, column, row, self.plugins.len()) {
            self.plugin_index = index;
        }
        self.set_focus(FocusPane::Plugins);
    }

    fn handle_navigation_click(&mut self, area: Rect, column: u16, row: u16) {
        let list_area = navigation_list_area(area);
        match self.navigation_tab {
            NavigationTab::Targets => {
                if let Some(index) = list_index_in_area(list_area, column, row, Target::all().len())
                {
                    self.target_index = index;
                    self.profile_index = 0;
                    self.set_focus(FocusPane::Targets);
                }
            }
            NavigationTab::Profiles => {
                if let Some(index) = list_index_in_area(
                    list_area,
                    column,
                    row,
                    self.selected_profile_indices().len(),
                ) {
                    self.profile_index = index;
                    self.set_focus(FocusPane::Profiles);
                }
            }
        }
    }

    fn set_focus(&mut self, focus: FocusPane) {
        match focus {
            FocusPane::Targets => self.navigation_tab = NavigationTab::Targets,
            FocusPane::Profiles => self.navigation_tab = NavigationTab::Profiles,
            FocusPane::Mcp => {
                if self.selected_mcp_refresh_state() == McpRefreshState::NotLoaded {
                    self.request_mcp_refresh_for(self.selected_target());
                }
            }
            FocusPane::Plugins => {}
            FocusPane::Details => {}
            FocusPane::PluginSearch => {}
        }
        self.focus = focus;
    }

    fn focus_next_left_pane(&mut self) {
        match self.focus {
            FocusPane::Targets => self.set_focus(FocusPane::Profiles),
            FocusPane::Profiles => self.set_focus(FocusPane::Mcp),
            FocusPane::Mcp => self.set_focus(FocusPane::Plugins),
            FocusPane::Plugins => self.set_focus(FocusPane::Targets),
            FocusPane::Details => self.set_focus(FocusPane::Targets),
            FocusPane::PluginSearch => self.set_focus(FocusPane::Targets),
        };
    }

    fn focus_previous_left_pane(&mut self) {
        match self.focus {
            FocusPane::Targets => self.set_focus(FocusPane::Plugins),
            FocusPane::Profiles => self.set_focus(FocusPane::Targets),
            FocusPane::Mcp => self.set_focus(FocusPane::Profiles),
            FocusPane::Plugins => self.set_focus(FocusPane::Mcp),
            FocusPane::Details => self.set_focus(FocusPane::Mcp),
            FocusPane::PluginSearch => self.set_focus(FocusPane::Mcp),
        };
    }

    fn select_numbered_pane(&mut self, value: char) {
        match value {
            '1' => {
                self.set_focus(FocusPane::Targets);
                self.message = "Focused targets".to_string();
            }
            '2' => {
                self.set_focus(FocusPane::Mcp);
                if self.selected_mcp_refresh_state() != McpRefreshState::Loading {
                    self.message = "Focused MCP servers".to_string();
                }
            }
            '3' => {
                self.set_focus(FocusPane::Plugins);
                self.message = "Focused plugins".to_string();
            }
            '0' => {
                self.set_focus(FocusPane::Details);
                self.message = "Focused details".to_string();
            }
            '4'..='5' => {
                self.message = format!("Panel {value} is reserved");
            }
            _ => {}
        }
    }

    fn move_selection(&mut self, delta: isize) {
        match self.focus {
            FocusPane::Targets => {
                self.target_index = move_index(self.target_index, Target::all().len(), delta);
                self.profile_index = 0;
            }
            FocusPane::Profiles => {
                self.profile_index = move_index(
                    self.profile_index,
                    self.selected_profile_indices().len(),
                    delta,
                );
            }
            FocusPane::Mcp => {
                self.mcp_index =
                    move_index(self.mcp_index, self.selected_mcp_indices().len(), delta);
            }
            FocusPane::Plugins => {
                self.plugin_index = move_index(self.plugin_index, self.plugins.len(), delta);
            }
            FocusPane::Details => {}
            FocusPane::PluginSearch => {
                self.plugin_search_index = move_index(
                    self.plugin_search_index,
                    self.filtered_available_plugin_indices().len(),
                    delta,
                );
            }
        }
    }

    fn selected_target(&self) -> Target {
        Target::all()[self.target_index]
    }

    pub(crate) fn selected_profile_indices(&self) -> Vec<usize> {
        let target = self.selected_target();
        let mut indices: Vec<usize> = self
            .config
            .profiles
            .iter()
            .enumerate()
            .filter_map(|(index, profile)| (profile.target == target).then_some(index))
            .collect();
        indices.sort_by(|left, right| {
            self.config.profiles[*left]
                .name
                .cmp(&self.config.profiles[*right].name)
        });
        indices
    }

    fn selected_profile_index_in_config(&self) -> Option<usize> {
        self.selected_profile_indices()
            .get(self.profile_index)
            .copied()
    }

    pub(crate) fn selected_profile(&self) -> Option<&Profile> {
        self.selected_profile_index_in_config()
            .and_then(|index| self.config.profiles.get(index))
    }

    pub(crate) fn selected_mcp_indices(&self) -> Vec<usize> {
        let target = self.selected_target();
        self.mcp_servers
            .iter()
            .enumerate()
            .filter_map(|(index, server)| (server.target == target).then_some(index))
            .collect()
    }

    pub(crate) fn selected_mcp(&self) -> Option<&McpServer> {
        self.selected_mcp_indices()
            .get(self.mcp_index)
            .and_then(|index| self.mcp_servers.get(*index))
    }

    pub(crate) fn selected_plugin(&self) -> Option<&Plugin> {
        self.plugins.get(self.plugin_index)
    }

    pub(crate) fn filtered_available_plugin_indices(&self) -> Vec<usize> {
        let query = self.plugin_search_query.trim().to_lowercase();
        self.available_plugins
            .iter()
            .enumerate()
            .filter_map(|(index, plugin)| {
                if query.is_empty() || plugin_matches_query(plugin, &query) {
                    Some(index)
                } else {
                    None
                }
            })
            .collect()
    }

    pub(crate) fn selected_available_plugin(&self) -> Option<&Plugin> {
        self.filtered_available_plugin_indices()
            .get(self.plugin_search_index)
            .and_then(|index| self.available_plugins.get(*index))
    }

    fn activate_selection(&mut self) -> TuiAction {
        match self.focus {
            FocusPane::Targets => {
                self.set_focus(FocusPane::Profiles);
                self.message = format!("Profiles for {}", self.selected_target());
                TuiAction::None
            }
            FocusPane::Profiles => self.use_selected_profile(),
            FocusPane::Mcp => TuiAction::None,
            FocusPane::Plugins => {
                self.toggle_selected_plugin();
                TuiAction::None
            }
            FocusPane::Details => TuiAction::None,
            FocusPane::PluginSearch => {
                self.install_or_confirm_selected_plugin();
                TuiAction::None
            }
        }
    }

    fn back_or_quit(&mut self) -> TuiAction {
        match self.focus {
            FocusPane::Profiles => {
                self.set_focus(FocusPane::Targets);
                self.message = "Back to targets".to_string();
                TuiAction::None
            }
            FocusPane::Mcp => {
                self.set_focus(FocusPane::Targets);
                self.message = "Back to targets".to_string();
                TuiAction::None
            }
            FocusPane::Plugins => {
                self.set_focus(FocusPane::Targets);
                self.message = "Back to targets".to_string();
                TuiAction::None
            }
            FocusPane::Details => {
                match self.navigation_tab {
                    NavigationTab::Targets => self.focus = FocusPane::Targets,
                    NavigationTab::Profiles => self.focus = FocusPane::Profiles,
                }
                TuiAction::None
            }
            FocusPane::PluginSearch => {
                self.set_focus(FocusPane::Plugins);
                TuiAction::None
            }
            FocusPane::Targets => TuiAction::Quit,
        }
    }

    fn use_selected_profile(&mut self) -> TuiAction {
        let Some(profile) = self.selected_profile() else {
            return TuiAction::None;
        };
        let name = profile.name.clone();
        let target = profile.target;
        match self.config.use_profile(&name, Some(target)) {
            Ok(()) => {
                self.message = format!("Using {name} for {target}");
                TuiAction::Save
            }
            Err(error) => {
                self.message = error.to_string();
                TuiAction::None
            }
        }
    }

    fn open_edit_form(&mut self) {
        let Some(profile) = self.selected_profile() else {
            return;
        };
        if is_builtin_profile(profile) {
            self.message = "Built-in profiles are read-only".to_string();
            return;
        }
        self.mode = TuiMode::Editing(ProfileForm::edit(profile));
    }

    fn open_delete_confirmation(&mut self) {
        let Some(profile) = self.selected_profile() else {
            return;
        };
        if is_builtin_profile(profile) {
            self.message = "Built-in profiles cannot be deleted".to_string();
            return;
        }
        self.mode = TuiMode::ConfirmDelete;
    }

    fn open_delete_mcp_confirmation(&mut self) {
        if self.selected_mcp().is_none() {
            self.message = "No MCP server selected".to_string();
            return;
        }
        self.mode = TuiMode::ConfirmDeleteMcp;
    }

    fn open_delete_plugin_confirmation(&mut self) {
        if self.selected_plugin().is_none() {
            self.message = "No plugin selected".to_string();
            return;
        }
        self.mode = TuiMode::ConfirmDeletePlugin;
    }

    pub(crate) fn save_form(&mut self, form: &mut ProfileForm) -> TuiAction {
        let profile = form.profile();
        if profile.name.is_empty() {
            form.error = Some("Name is required".to_string());
            self.mode = TuiMode::Editing(form.clone());
            return TuiAction::None;
        }
        if self.profile_name_exists(&profile, form.original.as_ref()) {
            form.error = Some(format!(
                "Profile '{}' already exists for {}",
                profile.name, profile.target
            ));
            self.mode = TuiMode::Editing(form.clone());
            return TuiAction::None;
        }

        match &form.original {
            Some(original) => {
                if let Some(existing) = self.config.profiles.iter_mut().find(|candidate| {
                    candidate.name == original.name && candidate.target == original.target
                }) {
                    *existing = profile.clone();
                    if self.config.is_current(original) {
                        self.config.set_current(CurrentProfile {
                            target: profile.target,
                            name: profile.name.clone(),
                        });
                    }
                    self.message = format!("Updated {} for {}", profile.name, profile.target);
                } else {
                    form.error = Some("Original profile no longer exists".to_string());
                    self.mode = TuiMode::Editing(form.clone());
                    return TuiAction::None;
                }
            }
            None => {
                if let Err(error) = self.config.add(profile.clone()) {
                    form.error = Some(error.to_string());
                    self.mode = TuiMode::Editing(form.clone());
                    return TuiAction::None;
                }
                self.profile_index = self
                    .selected_profile_indices()
                    .iter()
                    .position(|index| self.config.profiles[*index].name == profile.name)
                    .unwrap_or(self.profile_index);
                self.message = format!("Added {} for {}", profile.name, profile.target);
            }
        }

        self.mode = TuiMode::Normal;
        TuiAction::Save
    }

    fn profile_name_exists(&self, profile: &Profile, original: Option<&CurrentProfile>) -> bool {
        self.config.profiles.iter().any(|existing| {
            existing.name == profile.name
                && existing.target == profile.target
                && !original.is_some_and(|original| {
                    original.name == existing.name && original.target == existing.target
                })
        })
    }

    fn delete_selected_profile(&mut self) -> TuiAction {
        let Some(profile) = self.selected_profile() else {
            self.mode = TuiMode::Normal;
            return TuiAction::None;
        };
        if is_builtin_profile(profile) {
            self.message = "Built-in profiles cannot be deleted".to_string();
            self.mode = TuiMode::Normal;
            return TuiAction::None;
        }

        let name = profile.name.clone();
        let target = profile.target;
        match self.config.delete(&name, Some(target)) {
            Ok(()) => {
                self.profile_index = self
                    .profile_index
                    .min(self.selected_profile_indices().len().saturating_sub(1));
                self.mode = TuiMode::Normal;
                self.message = format!("Deleted {name} for {target}");
                TuiAction::Save
            }
            Err(error) => {
                self.mode = TuiMode::Normal;
                self.message = error.to_string();
                TuiAction::None
            }
        }
    }

    fn delete_selected_mcp(&mut self) {
        let Some(server) = self.selected_mcp().cloned() else {
            self.mode = TuiMode::Normal;
            return;
        };

        let Some(mcp) = tool_for(server.target).mcp() else {
            self.mode = TuiMode::Normal;
            self.message = format!("{} does not support MCP", server.target);
            return;
        };

        match mcp.remove_server(&server.name) {
            Ok(()) => {
                self.mode = TuiMode::Normal;
                self.message = format!("Deleted MCP {} for {}", server.name, server.target);
                self.request_mcp_refresh_for(server.target);
            }
            Err(error) => {
                self.mode = TuiMode::Normal;
                self.message = error.to_string();
            }
        }
    }

    fn toggle_selected_plugin(&mut self) {
        let Some(plugin) = self.selected_plugin().cloned() else {
            self.message = "No plugin selected".to_string();
            return;
        };
        let Some(capability) = tool_for(plugin.target).plugin() else {
            self.message = format!("{} does not support plugins", plugin.target);
            return;
        };
        let enabled = !plugin.enabled;
        match capability.set_plugin_enabled(&plugin.name, enabled) {
            Ok(()) => {
                self.refresh_plugins();
                self.message = format!(
                    "{} plugin {} for {}",
                    if enabled { "Enabled" } else { "Disabled" },
                    plugin.name,
                    plugin.target
                );
            }
            Err(error) => self.message = error.to_string(),
        }
    }

    fn delete_selected_plugin(&mut self) {
        let Some(plugin) = self.selected_plugin().cloned() else {
            self.mode = TuiMode::Normal;
            return;
        };
        let Some(capability) = tool_for(plugin.target).plugin() else {
            self.mode = TuiMode::Normal;
            self.message = format!("{} does not support plugins", plugin.target);
            return;
        };
        match capability.remove_plugin(&plugin.name) {
            Ok(()) => {
                self.mode = TuiMode::Normal;
                self.refresh_plugins();
                self.plugin_index = self.plugin_index.min(self.plugins.len().saturating_sub(1));
                self.message = format!("Deleted plugin {} for {}", plugin.name, plugin.target);
            }
            Err(error) => {
                self.mode = TuiMode::Normal;
                self.message = error.to_string();
            }
        }
    }

    fn install_or_confirm_selected_plugin(&mut self) {
        let Some(plugin) = self.selected_available_plugin().cloned() else {
            self.message = "No plugin selected".to_string();
            return;
        };
        if plugin.installed {
            self.mode = TuiMode::ConfirmUninstallPlugin(plugin);
            return;
        }
        let Some(capability) = tool_for(plugin.target).plugin() else {
            self.message = format!("{} does not support plugins", plugin.target);
            return;
        };
        match capability.install_plugin(&plugin.selector) {
            Ok(()) => {
                self.refresh_plugins();
                self.request_plugin_refresh_for(plugin.target);
                self.message = format!("Installed plugin {} for {}", plugin.name, plugin.target);
            }
            Err(error) => self.message = error.to_string(),
        }
    }

    fn uninstall_plugin(&mut self, plugin: Plugin) {
        let Some(capability) = tool_for(plugin.target).plugin() else {
            self.mode = TuiMode::Normal;
            self.message = format!("{} does not support plugins", plugin.target);
            return;
        };
        match capability.remove_plugin(&plugin.selector) {
            Ok(()) => {
                self.mode = TuiMode::Normal;
                self.refresh_plugins();
                self.request_plugin_refresh_for(plugin.target);
                self.message = format!("Uninstalled plugin {} for {}", plugin.name, plugin.target);
            }
            Err(error) => {
                self.mode = TuiMode::Normal;
                self.message = error.to_string();
            }
        }
    }

    fn refresh_plugins(&mut self) {
        self.plugins = load_plugins();
    }

    fn select_current_target(&mut self) {
        for (index, target) in Target::all().iter().enumerate() {
            let default = default_current_profile(*target);
            if self
                .config
                .current_for_target(*target)
                .is_some_and(|current| current.name != default.name)
            {
                self.target_index = index;
                return;
            }
        }
        self.target_index = 0;
    }
}

fn move_index(current: usize, len: usize, delta: isize) -> usize {
    if len == 0 {
        return 0;
    }

    let next = current as isize + delta;
    next.clamp(0, len.saturating_sub(1) as isize) as usize
}

fn target_refresh_index(target: Target) -> usize {
    Target::all()
        .into_iter()
        .position(|candidate| candidate == target)
        .expect("target should be known")
}

fn load_plugins() -> Vec<Plugin> {
    let mut plugins = Vec::new();
    for tool in crate::tools::all_tools() {
        let Some(capability) = tool.plugin() else {
            continue;
        };
        if let Ok(mut target_plugins) = capability.list_plugins() {
            plugins.append(&mut target_plugins);
        }
    }
    plugins
}

fn plugin_matches_query(plugin: &Plugin, query: &str) -> bool {
    plugin.name.to_lowercase().contains(query)
        || plugin.selector.to_lowercase().contains(query)
        || plugin
            .marketplace
            .as_deref()
            .unwrap_or_default()
            .to_lowercase()
            .contains(query)
        || plugin.target.to_string().to_lowercase().contains(query)
}

fn format_command_hints(hints: &[CommandHint]) -> String {
    hints
        .iter()
        .map(|hint| format!("{}: {}", hint.label, hint.keys))
        .collect::<Vec<_>>()
        .join(" | ")
}

pub(crate) fn is_builtin_profile(profile: &Profile) -> bool {
    matches!(
        (profile.target, profile.name.as_str()),
        (Target::Codex, DEFAULT_CODEX_PROFILE) | (Target::Claude, DEFAULT_CLAUDE_PROFILE)
    )
}
