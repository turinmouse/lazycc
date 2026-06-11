use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

use crate::config::{
    Config, CurrentProfile, DEFAULT_CLAUDE_PROFILE, DEFAULT_CODEX_PROFILE, Profile, Target,
    default_current_profile,
};

use super::layout::{
    FORM_FIELD_COUNT, TuiLayout, centered_rect, form_layout, list_index_in_area,
    navigation_list_area, rect_contains, tui_layout,
};
use super::theme::{TuiTheme, TuiThemeKind};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FocusPane {
    Targets,
    Profiles,
    Details,
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
            mode: TuiMode::Normal,
            theme_kind: TuiThemeKind::Classic,
            message: "Enter opens profiles, Esc backs out, n adds, t theme, q quits".to_string(),
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
        }
    }

    pub(crate) fn handle_mouse(&mut self, mouse: MouseEvent, area: Rect) -> TuiAction {
        match self.mode.clone() {
            TuiMode::Normal => self.handle_normal_mouse(mouse, area),
            TuiMode::Editing(mut form) => self.handle_form_mouse(mouse, area, &mut form),
            TuiMode::ConfirmDelete => self.handle_delete_mouse(mouse, area),
        }
    }

    pub(crate) fn theme(&self) -> TuiTheme {
        self.theme_kind.theme()
    }

    fn handle_normal_key(&mut self, key: KeyEvent) -> TuiAction {
        match key.code {
            KeyCode::Char('q') => TuiAction::Quit,
            KeyCode::Esc => self.back_or_quit(),
            KeyCode::Tab | KeyCode::BackTab => TuiAction::None,
            KeyCode::Left => {
                self.focus_previous_left_pane();
                TuiAction::None
            }
            KeyCode::Right => {
                self.focus_next_left_pane();
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
                self.mode = TuiMode::Editing(ProfileForm::add(self.selected_target()));
                TuiAction::None
            }
            KeyCode::Char('e') => {
                self.open_edit_form();
                TuiAction::None
            }
            KeyCode::Char('d') => {
                self.open_delete_confirmation();
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

    fn handle_normal_mouse(&mut self, mouse: MouseEvent, area: Rect) -> TuiAction {
        let layout = tui_layout(area);
        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if rect_contains(layout.navigation, mouse.column, mouse.row) {
                    self.handle_navigation_click(layout.navigation, mouse.column, mouse.row);
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

    fn focus_from_mouse(&mut self, layout: TuiLayout, column: u16, row: u16) -> bool {
        if rect_contains(layout.navigation, column, row) {
            match self.navigation_tab {
                NavigationTab::Targets => self.set_focus(FocusPane::Targets),
                NavigationTab::Profiles => self.set_focus(FocusPane::Profiles),
            }
            true
        } else {
            false
        }
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
            FocusPane::Details => {}
        }
        self.focus = focus;
    }

    fn focus_next_left_pane(&mut self) {
        match self.focus {
            FocusPane::Targets => self.set_focus(FocusPane::Profiles),
            FocusPane::Profiles => self.set_focus(FocusPane::Targets),
            FocusPane::Details => self.set_focus(FocusPane::Targets),
        };
    }

    fn focus_previous_left_pane(&mut self) {
        match self.focus {
            FocusPane::Targets => self.set_focus(FocusPane::Profiles),
            FocusPane::Profiles => self.set_focus(FocusPane::Targets),
            FocusPane::Details => self.set_focus(FocusPane::Profiles),
        };
    }

    fn select_numbered_pane(&mut self, value: char) {
        match value {
            '1' => {
                self.set_focus(FocusPane::Targets);
                self.message = "Focused targets".to_string();
            }
            '2' => {
                self.set_focus(FocusPane::Profiles);
                self.message = "Focused profiles".to_string();
            }
            '0' => {
                self.focus = FocusPane::Details;
                self.message = "Focused details".to_string();
            }
            '3'..='5' => {
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
            FocusPane::Details => {}
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

    fn activate_selection(&mut self) -> TuiAction {
        match self.focus {
            FocusPane::Targets => {
                self.set_focus(FocusPane::Profiles);
                self.message = format!("Profiles for {}", self.selected_target());
                TuiAction::None
            }
            FocusPane::Profiles => self.use_selected_profile(),
            FocusPane::Details => TuiAction::None,
        }
    }

    fn back_or_quit(&mut self) -> TuiAction {
        match self.focus {
            FocusPane::Profiles => {
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

pub(crate) fn is_builtin_profile(profile: &Profile) -> bool {
    matches!(
        (profile.target, profile.name.as_str()),
        (Target::Codex, DEFAULT_CODEX_PROFILE) | (Target::Claude, DEFAULT_CLAUDE_PROFILE)
    )
}
