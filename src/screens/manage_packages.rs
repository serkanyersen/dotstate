use crate::components::footer::Footer;
use crate::components::header::Header;
use crate::config::Config;
use crate::keymap::{Action, Keymap};
use crate::screens::{RenderContext, Screen, ScreenAction, ScreenContext};
use crate::services::{PackageCreationParams, PackageService};
use crate::styles::{theme, LIST_HIGHLIGHT_SYMBOL};
use crate::ui::{
    AddPackageField, InstallationStatus, InstallationStep, PackageManagerState, PackagePopupType,
    PackageStatus, Screen as ScreenEnum,
};
use crate::utils::package_installer::PackageInstaller;
use crate::utils::package_manager::PackageManagerImpl;
use crate::utils::profile_manifest::{Package, PackageManager};
use crate::utils::{
    create_standard_layout, focused_border_style, unfocused_border_style, MouseRegions,
};
use crate::widgets::{TextInputWidget, TextInputWidgetExt};
use anyhow::Result;
use crossterm::event::{Event, KeyCode, KeyModifiers, MouseButton, MouseEventKind};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, Padding, Paragraph, Tabs, Wrap};
use std::time::Duration;
use tracing::{debug, error, info, warn};

pub struct ManagePackagesScreen {
    pub state: PackageManagerState,
    /// Mouse regions for package list items (value = package index)
    mouse_regions: MouseRegions<usize>,
    /// Stored list pane area for scroll hit-testing
    list_pane_area: Option<Rect>,
    /// Add/Edit popup: clickable field areas (area, field) for click-to-focus
    add_field_areas: Vec<(Rect, AddPackageField)>,
    /// Import popup: tabs area for click-to-switch-tab
    import_tabs_area: Option<Rect>,
    /// Import popup: filter input area for click-to-focus
    import_filter_area: Option<Rect>,
    /// Import popup: list area for click/scroll
    import_list_area: Option<Rect>,
    /// Import popup: clickable list item regions (value = filtered index)
    import_list_regions: MouseRegions<usize>,
}

impl Default for ManagePackagesScreen {
    fn default() -> Self {
        Self::new()
    }
}

impl ManagePackagesScreen {
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: PackageManagerState::default(),
            mouse_regions: MouseRegions::new(),
            list_pane_area: None,
            add_field_areas: Vec::new(),
            import_tabs_area: None,
            import_filter_area: None,
            import_list_area: None,
            import_list_regions: MouseRegions::new(),
        }
    }

    pub fn get_state_mut(&mut self) -> &mut PackageManagerState {
        &mut self.state
    }

    pub fn update_packages(&mut self, packages: Vec<Package>, active_profile: &str) {
        self.state.packages = packages;
        self.state.active_profile = active_profile.to_string();

        // Initialize statuses from cache
        let mut statuses = Vec::with_capacity(self.state.packages.len());
        for package in &self.state.packages {
            if let Some(entry) = self.state.cache.get_status(active_profile, &package.name) {
                if entry.installed {
                    statuses.push(PackageStatus::Installed);
                } else {
                    statuses.push(PackageStatus::NotInstalled);
                }
            } else {
                statuses.push(PackageStatus::Unknown);
            }
        }
        self.state.package_statuses = statuses;
    }

    pub fn reset_state(&mut self) {
        self.state.installation_step = InstallationStep::NotStarted;
        self.state.installation_output.clear();
        self.state.installation_output_scroll = 0;
        self.state.popup_type = PackagePopupType::None;
    }

    pub fn start_checking(&mut self) {
        let state = &mut self.state;
        state.is_checking = true;
        state.checking_index = None;
        state.checking_delay_until = Some(std::time::Instant::now() + Duration::from_millis(100));
        // Don't reset statuses here - they are initialized by update_packages (potentially from cache)
        // Only packages with Unknown status will be checked by process_package_check_step
    }

    pub fn start_installing_missing_packages(&mut self) {
        let state = &mut self.state;
        let mut packages_to_install = Vec::new();
        for (idx, status) in state.package_statuses.iter().enumerate() {
            if matches!(status, PackageStatus::NotInstalled) {
                packages_to_install.push(idx);
            }
        }

        if !packages_to_install.is_empty() {
            let first_idx = packages_to_install[0];
            let package_name = if let Some(p) = state.packages.get(first_idx) {
                p.name.clone()
            } else {
                "Unknown".to_string()
            };
            let total = packages_to_install.len();
            let remaining = packages_to_install[1..].to_vec();

            state.installation_step = InstallationStep::Installing {
                package_index: first_idx,
                package_name,
                total_packages: total,
                packages_to_install: remaining,
                installed: Vec::new(),
                failed: Vec::new(),
                status_rx: None,
            };
            state.installation_output.clear();
            state.installation_output_scroll = 0;
            state.installation_delay_until =
                Some(std::time::Instant::now() + Duration::from_millis(100));
        }
    }

    fn get_action(&self, key: KeyCode, modifiers: KeyModifiers, keymap: &Keymap) -> Option<Action> {
        keymap.get_action(key, modifiers)
    }

    /// Process periodic tasks (package checking, installation monitoring, import discovery)
    /// Returns `ScreenAction::Refresh` if a redraw is needed
    pub fn tick(&mut self) -> Result<ScreenAction> {
        let mut needs_redraw = false;

        // 1. Process Package Checking
        if self.state.is_checking {
            // Check delay
            if let Some(delay_until) = self.state.checking_delay_until {
                if std::time::Instant::now() >= delay_until {
                    self.state.checking_delay_until = None;
                    self.process_package_check_step()?;
                    needs_redraw = true;
                }
            } else {
                self.process_package_check_step()?;
                needs_redraw = true;
            }
        }

        // 2. Process Installation
        if !matches!(self.state.installation_step, InstallationStep::NotStarted) {
            self.process_installation_step()?;
            needs_redraw = true;
        }

        // 3. Process Import Discovery (async polling)
        if self.state.import_loading {
            // Tick the spinner animation
            self.state.import_spinner_tick = self.state.import_spinner_tick.wrapping_add(1);
            // Poll async discovery (non-blocking)
            self.poll_import_discovery();
            needs_redraw = true;
        }

        if needs_redraw {
            Ok(ScreenAction::Refresh)
        } else {
            Ok(ScreenAction::None)
        }
    }

    fn process_package_check_step(&mut self) -> Result<()> {
        let state = &mut self.state;

        if state.packages.is_empty() {
            state.is_checking = false;
            return Ok(());
        }

        // Initialize statuses if needed
        if state.package_statuses.len() != state.packages.len() {
            state.package_statuses = vec![PackageStatus::Unknown; state.packages.len()];
        }

        // STEP 1: If we have a target package to check, check it now.
        if let Some(index) = state.checking_index {
            if index < state.packages.len() {
                let package = &state.packages[index];
                debug!("Checking package: {} (index: {})", package.name, index);

                let check_result = PackageInstaller::check_exists(package);
                let pkg_name = package.name.clone();
                let pkg_manager = package.manager.clone();

                match check_result {
                    Ok((exists, check_cmd, output)) => {
                        // Update cache
                        if !state.active_profile.is_empty() {
                            if let Err(e) = state.cache.update_status(
                                &state.active_profile,
                                &pkg_name,
                                exists,
                                check_cmd.clone(),
                                output.clone(),
                            ) {
                                warn!("Failed to update package cache: {}", e);
                            }
                        }

                        if exists {
                            state.package_statuses[index] = PackageStatus::Installed;
                        } else if !PackageManagerImpl::is_manager_installed(&pkg_manager) {
                            state.package_statuses[index] = PackageStatus::Error(format!(
                                "Package not found and package manager '{pkg_manager:?}' is not installed"
                            ));
                        } else {
                            state.package_statuses[index] = PackageStatus::NotInstalled;
                        }
                    }
                    Err(e) => {
                        error!("Error checking package {}: {}", pkg_name, e);
                        state.package_statuses[index] = PackageStatus::Error(e.to_string());
                    }
                }
            }

            // Store the index we just checked before clearing it
            let checked_index = index;
            state.checking_index = None;

            info!(
                "Finished checking selected package at index {}",
                checked_index
            );
            return Ok(());
        }

        // STEP 2: Look for more work (next 'Unknown' package)
        // This only runs for "check all" mode (when checking_index was None initially)
        if let Some(index) = state
            .package_statuses
            .iter()
            .position(|s| matches!(s, PackageStatus::Unknown))
        {
            // Found work: Set checking_index so the loading icon shows for this package
            state.checking_index = Some(index);

            // Schedule delay to let UI render the Loading icon for this index before we check it
            state.checking_delay_until =
                Some(std::time::Instant::now() + Duration::from_millis(10));
            return Ok(());
        }

        // STEP 3: No work left
        state.is_checking = false;
        state.checking_index = None;
        info!("Finished checking all packages");

        // Check if we just finished checking a newly added package
        // If it's not installed, prompt the user to install it
        if let Some(new_idx) = state.newly_added_index.take() {
            if let Some(status) = state.package_statuses.get(new_idx) {
                if matches!(status, PackageStatus::NotInstalled) {
                    info!(
                        "Newly added package at index {} is not installed, prompting to install",
                        new_idx
                    );
                    state.popup_type = PackagePopupType::InstallMissing;
                }
            }
        }

        Ok(())
    }

    fn process_installation_step(&mut self) -> Result<()> {
        // Logic ported from app.rs process_installation_step
        // We'll need to implement the async parts. The tricky part is the Receiver.
        // It's part of the state enum InstallationStep::Installing { status_rx, ... }
        // But `PackageManagerState` in `ui_state.rs` defines `status_rx` as `Option<Receiver<...>>`.
        // So we can access it here!

        let state = &mut self.state;
        if let InstallationStep::Installing {
            package_index,
            package_name,
            total_packages: _,
            packages_to_install: _,
            installed,
            failed,
            status_rx,
        } = &mut state.installation_step
        {
            // Check for initial delay
            if let Some(delay) = state.installation_delay_until {
                if std::time::Instant::now() < delay {
                    return Ok(());
                }
                state.installation_delay_until = None;

                // Start installation if not started (rx is None)
                if status_rx.is_none() {
                    info!("Starting installation for package: {}", package_name);
                    // Need to get the actual package
                    let pkg = if let Some(p) = state.packages.get(*package_index) {
                        p.clone()
                    } else {
                        // Error case
                        error!("Package index {} out of bounds", package_index);
                        failed.push((*package_index, "Package index out of bounds".to_string()));
                        // Move to next
                        self.advance_installation()?;
                        return Ok(());
                    };

                    // Spawn installation thread
                    let (tx, rx) = std::sync::mpsc::channel();
                    let pkg_clone = pkg.clone();
                    std::thread::spawn(move || {
                        PackageInstaller::install(&pkg_clone, tx);
                    });
                    *status_rx = Some(rx);
                }
            }

            // Check for result
            let mut finished_current = false;
            if let Some(rx) = status_rx {
                // Drain available messages
                loop {
                    match rx.try_recv() {
                        Ok(result) => {
                            match result {
                                InstallationStatus::Output(line) => {
                                    state.installation_output.push(line);
                                }
                                InstallationStatus::Complete { success, error } => {
                                    finished_current = true;
                                    if success {
                                        info!("Successfully installed {}", package_name);
                                        installed.push(*package_index);
                                        state
                                            .installation_output
                                            .push(format!("✅ Installed {package_name}"));
                                        // Update status in list
                                        if *package_index < state.package_statuses.len() {
                                            state.package_statuses[*package_index] =
                                                PackageStatus::Installed;
                                        }

                                        // Update cache
                                        if !state.active_profile.is_empty() {
                                            if let Err(e) = state.cache.update_status(
                                                &state.active_profile,
                                                package_name,
                                                true,
                                                None,
                                                Some("Successfully installed".to_string()),
                                            ) {
                                                warn!("Failed to update package cache: {}", e);
                                            }
                                        }
                                    } else {
                                        let err_msg =
                                            error.unwrap_or_else(|| "Unknown error".to_string());
                                        error!("Failed to install {}: {}", package_name, err_msg);
                                        failed.push((*package_index, err_msg.clone()));
                                        state.installation_output.push(format!(
                                            "❌ Failed to install {package_name}: {err_msg}"
                                        ));
                                    }
                                    // Break after Complete to avoid hitting Disconnected
                                    break;
                                }
                            }
                        }
                        Err(std::sync::mpsc::TryRecvError::Empty) => {
                            break;
                        }
                        Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                            // Thread died?
                            finished_current = true;
                            failed.push((
                                *package_index,
                                "Installation thread disconnected".to_string(),
                            ));
                            break;
                        }
                    }
                }
            }

            if finished_current {
                self.advance_installation()?;
            }
        } else if let InstallationStep::Complete { .. } = &state.installation_step {
            // Already complete, nothing to tick
        }

        Ok(())
    }

    fn advance_installation(&mut self) -> Result<()> {
        // Logic to move to next package or finish
        // We have to destruct and reconstruct state carefully due to ownership
        // Or strictly manipulate the fields inside the match guard if we used a struct, but it's an enum.
        // We'll clone necessary implementation details to move state forward.

        // We are inside `process_installation_step` which has mutable borrow.
        // Calling this helper requires splitting borrows or careful logic.
        // Let's implement logic inline or carefully.
        // Actually, simpler: clone 'packages_to_install', 'installed', 'failed' etc. then re-assign state.

        let state = &mut self.state;
        // Extract needed data to decide next step
        let (next_packages, installed_list, failed_list, total) =
            if let InstallationStep::Installing {
                packages_to_install,
                installed,
                failed,
                total_packages,
                ..
            } = &state.installation_step
            {
                (
                    packages_to_install.clone(),
                    installed.clone(),
                    failed.clone(),
                    *total_packages,
                )
            } else {
                return Ok(());
            };

        if next_packages.is_empty() {
            // Done
            state.installation_step = InstallationStep::Complete {
                installed: installed_list,
                failed: failed_list,
            };
        } else {
            // Process next
            let next_idx = next_packages[0];
            let remaining = next_packages[1..].to_vec();
            let pkg_name = if let Some(p) = state.packages.get(next_idx) {
                p.name.clone()
            } else {
                "Unknown".to_string()
            };

            state.installation_step = InstallationStep::Installing {
                package_index: next_idx,
                package_name: pkg_name,
                total_packages: total,
                packages_to_install: remaining,
                installed: installed_list,
                failed: failed_list,
                status_rx: None,
            };
            state.installation_delay_until =
                Some(std::time::Instant::now() + Duration::from_millis(500));
        }

        Ok(())
    }
}

impl Screen for ManagePackagesScreen {
    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: &RenderContext) -> Result<()> {
        // Background
        let t = theme();
        let background = Block::default().style(t.background_style());
        frame.render_widget(background, area);

        let config = ctx.config;

        // Ensure list state is initialized if we have packages
        if !self.state.packages.is_empty() && self.state.list_state.selected().is_none() {
            self.state.list_state.select(Some(0));
        }

        // Always render main content first (so dialogs can dim it)
        let layout = create_standard_layout(area, 5, 3);

        // Header
        let _header_height = Header::render(
            frame,
            layout.0,
            "DotState - Manage Packages",
            "Manage CLI tools and dependencies for your profile",
        )?;

        // Main content area
        let main_area = layout.1;

        // Split main area into left (list) and right (details) panels
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(main_area);

        // Left panel: Package list
        self.render_package_list(frame, chunks[0], config)?;

        // Right panel: Package details
        self.render_package_details(frame, chunks[1], config)?;

        // Footer
        let footer_text = if self.state.is_checking {
            "Checking packages...".to_string()
        } else if !matches!(self.state.installation_step, InstallationStep::NotStarted) {
            "Installing packages...".to_string()
        } else {
            let k = |a| config.keymap.get_key_display_for_action(a);
            format!(
                "{}: Navigate | {}: Add | {}: Import | {}: Edit | {}: Delete | {}: Check | {}: Install | {}: Back",
                config.keymap.navigation_display(),
                k(crate::keymap::Action::Create),
                k(crate::keymap::Action::Import),
                k(crate::keymap::Action::Edit),
                k(crate::keymap::Action::Delete),
                k(crate::keymap::Action::Refresh),
                k(crate::keymap::Action::Install),
                k(crate::keymap::Action::Cancel)
            )
        };
        Footer::render(frame, layout.2, &footer_text)?;

        // Render installation dialogs on top of content (with dimming)
        if !matches!(self.state.installation_step, InstallationStep::NotStarted) {
            self.render_installation_progress(frame, area)?;
        }

        // Render popups on top of the content (not instead of it)
        if self.state.popup_type != PackagePopupType::None {
            self.render_popup(frame, area, config)?;
        }

        Ok(())
    }

    fn handle_event(&mut self, event: Event, ctx: &ScreenContext) -> Result<ScreenAction> {
        let config = ctx.config;

        // If installation is in progress (and not complete), blocking input except maybe quit?
        // App.rs checks `installation_step` to show progress.
        if matches!(
            self.state.installation_step,
            InstallationStep::Installing { .. }
        ) {
            // Block generic input, maybe allow generic Quit via App?
            // But actually, we just ignore most keys.
            return Ok(ScreenAction::None);
        }

        // If installation complete, any key closes it
        if let InstallationStep::Complete { .. } = self.state.installation_step {
            if let Event::Key(_) = event {
                self.state.installation_step = InstallationStep::NotStarted;
                self.state.installation_output.clear();
                self.state.installation_output_scroll = 0;
                return Ok(ScreenAction::Refresh);
            }
            return Ok(ScreenAction::None);
        }

        // Popups consume all events
        if self.state.popup_type != PackagePopupType::None {
            match event {
                Event::Key(key) => match self.state.popup_type {
                    PackagePopupType::Add | PackagePopupType::Edit => {
                        return self.handle_add_edit_popup_event(key, config);
                    }
                    PackagePopupType::Delete => {
                        return self.handle_delete_popup_event(key, config);
                    }
                    PackagePopupType::InstallMissing => {
                        if let Some(action) =
                            self.get_action(key.code, key.modifiers, &config.keymap)
                        {
                            match action {
                                Action::Confirm => {
                                    self.state.popup_type = PackagePopupType::None;
                                    return Ok(ScreenAction::InstallMissingPackages);
                                }
                                Action::Cancel | Action::Quit => {
                                    self.state.popup_type = PackagePopupType::None;
                                    return Ok(ScreenAction::Refresh);
                                }
                                _ => {}
                            }
                        }
                        return Ok(ScreenAction::None);
                    }
                    PackagePopupType::Import => {
                        return self.handle_import_popup_event(key, config);
                    }
                    PackagePopupType::None => unreachable!(),
                },
                Event::Mouse(mouse) => {
                    return self.handle_popup_mouse_event(mouse);
                }
                _ => {}
            }
            return Ok(ScreenAction::None);
        }

        match event {
            Event::Key(key) => {
                if let Some(action) = self.get_action(key.code, key.modifiers, &config.keymap) {
                    return self.handle_main_list_action(action);
                }
            }
            Event::Mouse(mouse) => {
                return self.handle_mouse_event(mouse);
            }
            _ => {}
        }
        Ok(ScreenAction::None)
    }

    fn is_input_focused(&self) -> bool {
        matches!(
            self.state.popup_type,
            PackagePopupType::Add
                | PackagePopupType::Edit
                | PackagePopupType::Delete
                | PackagePopupType::Import
        )
    }
}

impl ManagePackagesScreen {
    fn handle_mouse_event(&mut self, mouse: crossterm::event::MouseEvent) -> Result<ScreenAction> {
        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some(&idx) = self.mouse_regions.hit_test(mouse.column, mouse.row) {
                    if !self.state.is_checking {
                        self.state.list_state.select(Some(idx));
                        return Ok(ScreenAction::Refresh);
                    }
                }
            }
            MouseEventKind::ScrollDown => {
                if let Some(area) = self.list_pane_area {
                    let pos = ratatui::layout::Position::new(mouse.column, mouse.row);
                    if area.contains(pos) && !self.state.is_checking {
                        for _ in 0..3 {
                            self.state.list_state.select_next();
                        }
                        return Ok(ScreenAction::Refresh);
                    }
                }
            }
            MouseEventKind::ScrollUp => {
                if let Some(area) = self.list_pane_area {
                    let pos = ratatui::layout::Position::new(mouse.column, mouse.row);
                    if area.contains(pos) && !self.state.is_checking {
                        for _ in 0..3 {
                            self.state.list_state.select_previous();
                        }
                        return Ok(ScreenAction::Refresh);
                    }
                }
            }
            _ => {}
        }
        Ok(ScreenAction::None)
    }

    fn handle_popup_mouse_event(
        &mut self,
        mouse: crossterm::event::MouseEvent,
    ) -> Result<ScreenAction> {
        use crate::ui::ImportFocus;

        let pos = ratatui::layout::Position::new(mouse.column, mouse.row);

        match self.state.popup_type {
            PackagePopupType::Add | PackagePopupType::Edit => {
                if let MouseEventKind::Down(MouseButton::Left) = mouse.kind {
                    for &(area, field) in &self.add_field_areas {
                        if area.contains(pos) {
                            self.state.add_focused_field = field;
                            return Ok(ScreenAction::Refresh);
                        }
                    }
                }
            }
            PackagePopupType::Import => match mouse.kind {
                MouseEventKind::Down(MouseButton::Left) => {
                    // Click on tabs area → focus tabs and select tab by x-position
                    if let Some(area) = self.import_tabs_area {
                        if area.contains(pos) {
                            self.state.import_focus = ImportFocus::Tabs;
                            // Calculate which tab was clicked based on x-position
                            // Each tab label is "  {name}  " with " " divider
                            let mut x_offset = area.x;
                            for (i, source) in
                                self.state.import_available_sources.iter().enumerate()
                            {
                                let tab_width = (source.display_name().len() + 4) as u16; // "  name  "
                                let divider_width = if i > 0 { 1 } else { 0 }; // " " divider
                                let start = x_offset + divider_width;
                                let end = start + tab_width;
                                if mouse.column >= start && mouse.column < end {
                                    if i != self.state.import_active_tab {
                                        self.switch_import_tab(i);
                                    }
                                    return Ok(ScreenAction::Refresh);
                                }
                                x_offset = end;
                            }
                            return Ok(ScreenAction::Refresh);
                        }
                    }
                    // Click on filter area → focus filter
                    if let Some(area) = self.import_filter_area {
                        if area.contains(pos) {
                            self.state.import_focus = ImportFocus::Filter;
                            return Ok(ScreenAction::Refresh);
                        }
                    }
                    // Click on list item → focus list and select item
                    if let Some(&filtered_idx) =
                        self.import_list_regions.hit_test(mouse.column, mouse.row)
                    {
                        self.state.import_focus = ImportFocus::List;
                        self.state.import_list_state.select(Some(filtered_idx));
                        return Ok(ScreenAction::Refresh);
                    }
                    // Click anywhere in list area → focus list
                    if let Some(area) = self.import_list_area {
                        if area.contains(pos) {
                            self.state.import_focus = ImportFocus::List;
                            return Ok(ScreenAction::Refresh);
                        }
                    }
                }
                MouseEventKind::ScrollDown => {
                    if let Some(area) = self.import_list_area {
                        if area.contains(pos) {
                            self.state.import_focus = ImportFocus::List;
                            let filtered = self.get_filtered_import_packages();
                            if !filtered.is_empty() {
                                let current = self.state.import_list_state.selected().unwrap_or(0);
                                let new_idx = (current + 3).min(filtered.len().saturating_sub(1));
                                self.state.import_list_state.select(Some(new_idx));
                            }
                            return Ok(ScreenAction::Refresh);
                        }
                    }
                }
                MouseEventKind::ScrollUp => {
                    if let Some(area) = self.import_list_area {
                        if area.contains(pos) {
                            self.state.import_focus = ImportFocus::List;
                            let current = self.state.import_list_state.selected().unwrap_or(0);
                            let new_idx = current.saturating_sub(3);
                            self.state.import_list_state.select(Some(new_idx));
                            return Ok(ScreenAction::Refresh);
                        }
                    }
                }
                _ => {}
            },
            _ => {}
        }

        Ok(ScreenAction::None)
    }

    fn handle_main_list_action(&mut self, action: Action) -> Result<ScreenAction> {
        let state = &mut self.state;
        match action {
            Action::MoveUp => {
                if !state.is_checking {
                    state.list_state.select_previous();
                    return Ok(ScreenAction::Refresh);
                }
            }
            Action::MoveDown => {
                if !state.is_checking {
                    state.list_state.select_next();
                    return Ok(ScreenAction::Refresh);
                }
            }
            Action::Refresh => {
                // Check All
                if state.popup_type == PackagePopupType::None
                    && !state.is_checking
                    && !state.packages.is_empty()
                {
                    // Initialize check all
                    if state.package_statuses.len() != state.packages.len() {
                        state.package_statuses = vec![PackageStatus::Unknown; state.packages.len()];
                    }
                    state.package_statuses = vec![PackageStatus::Unknown; state.packages.len()];
                    state.is_checking = true;
                    state.checking_index = None;
                    state.checking_delay_until =
                        Some(std::time::Instant::now() + Duration::from_millis(100));
                    return Ok(ScreenAction::Refresh);
                }
            }
            Action::CheckStatus => {
                // Check Selected
                if state.popup_type == PackagePopupType::None && !state.is_checking {
                    if let Some(idx) = state.list_state.selected() {
                        if idx < state.packages.len() {
                            // Reset status for this one
                            if state.package_statuses.len() != state.packages.len() {
                                state.package_statuses =
                                    vec![PackageStatus::Unknown; state.packages.len()];
                            }
                            state.package_statuses[idx] = PackageStatus::Unknown;
                            state.is_checking = true;
                            state.checking_index = Some(idx);
                            // Mark that we're checking only the selected package (not all)
                            // We'll use checking_index = Some(idx) to indicate "check selected" mode
                            state.checking_delay_until =
                                Some(std::time::Instant::now() + Duration::from_millis(100));
                            return Ok(ScreenAction::Refresh);
                        }
                    }
                }
            }
            Action::Install => {
                // Install Missing
                if state.popup_type == PackagePopupType::None && !state.is_checking {
                    // Logic usually just starts installing.
                    // We can check if any are missing and trigger the InstallMissingPackages action
                    // which App will handle (or we handle internally if we can).
                    // Wait, we moved 'process_installation_step' here. So we can start it here!
                    // But we need to detect WHICH packages are missing.

                    let missing_count = state
                        .package_statuses
                        .iter()
                        .filter(|s| matches!(s, PackageStatus::NotInstalled))
                        .count();
                    if missing_count > 0 {
                        // Trigger installation logic
                        return Ok(ScreenAction::InstallMissingPackages);
                    }
                }
            }
            Action::Create => {
                if state.popup_type == PackagePopupType::None && !state.is_checking {
                    self.start_add_package()?;
                    return Ok(ScreenAction::Refresh);
                }
            }
            Action::Edit => {
                if state.popup_type == PackagePopupType::None && !state.is_checking {
                    if let Some(idx) = state.list_state.selected() {
                        if idx < state.packages.len() {
                            self.start_edit_package(idx)?;
                            return Ok(ScreenAction::Refresh);
                        }
                    }
                }
            }
            Action::Delete => {
                if state.popup_type == PackagePopupType::None && !state.is_checking {
                    if let Some(idx) = state.list_state.selected() {
                        if idx < state.packages.len() {
                            state.delete_index = Some(idx);
                            state.popup_type = PackagePopupType::Delete;
                            state.delete_confirm_input.clear();
                            return Ok(ScreenAction::Refresh);
                        }
                    }
                }
            }
            Action::Cancel | Action::Quit => {
                if !state.is_checking {
                    return Ok(ScreenAction::Navigate(ScreenEnum::MainMenu));
                }
                // If checking, maybe cancel check?
                if state.is_checking {
                    state.is_checking = false;
                    return Ok(ScreenAction::Refresh);
                }
            }
            Action::Import => {
                if state.popup_type == PackagePopupType::None && !state.is_checking {
                    self.start_import()?;
                    return Ok(ScreenAction::Refresh);
                }
            }
            _ => {}
        }
        Ok(ScreenAction::None)
    }

    fn start_add_package(&mut self) -> Result<()> {
        let state = &mut self.state;
        state.popup_type = PackagePopupType::Add;
        state.add_editing_index = None;
        state.add_validation_error = None;
        state.add_name_input.clear();
        state.add_description_input.clear();
        state.add_package_name_input.clear();
        state.add_binary_name_input.clear();
        state.add_install_command_input.clear();
        state.add_existence_check_input.clear();
        state.add_manager_check_input.clear();
        state.add_focused_field = AddPackageField::Name;
        // Initialize managers
        state.available_managers = PackageManagerImpl::get_available_managers();
        if !state.available_managers.is_empty() {
            state.add_manager = Some(state.available_managers[0].clone());
            state.add_manager_selected = 0;
            state.add_is_custom = matches!(
                state.available_managers[0],
                crate::utils::profile_manifest::PackageManager::Custom
            );
        }
        state.manager_list_state.select(Some(0));
        Ok(())
    }

    fn start_edit_package(&mut self, index: usize) -> Result<()> {
        let state = &mut self.state;
        if let Some(pkg) = state.packages.get(index) {
            state.popup_type = PackagePopupType::Edit;
            state.add_editing_index = Some(index);
            state.add_validation_error = None;
            state.add_name_input = crate::utils::TextInput::with_text(&pkg.name);
            state.add_description_input =
                crate::utils::TextInput::with_text(pkg.description.clone().unwrap_or_default());
            state.add_package_name_input =
                crate::utils::TextInput::with_text(pkg.package_name.clone().unwrap_or_default());
            state.add_binary_name_input = crate::utils::TextInput::with_text(&pkg.binary_name);
            state.add_install_command_input =
                crate::utils::TextInput::with_text(pkg.install_command.clone().unwrap_or_default());
            state.add_existence_check_input =
                crate::utils::TextInput::with_text(pkg.existence_check.clone().unwrap_or_default());
            state.add_manager_check_input =
                crate::utils::TextInput::with_text(pkg.manager_check.clone().unwrap_or_default());

            state.available_managers = PackageManagerImpl::get_available_managers();
            state.add_manager = Some(pkg.manager.clone());
            state.add_is_custom = matches!(
                pkg.manager,
                crate::utils::profile_manifest::PackageManager::Custom
            );
            if let Some(pos) = state
                .available_managers
                .iter()
                .position(|m| *m == pkg.manager)
            {
                state.add_manager_selected = pos;
            } else {
                state.add_manager_selected = 0;
            }
            state
                .manager_list_state
                .select(Some(state.add_manager_selected));

            state.add_focused_field = AddPackageField::Name;
        }
        Ok(())
    }

    fn start_import(&mut self) -> Result<()> {
        use crate::services::PackageService;
        use crate::ui::ImportFocus;
        use crate::utils::{DiscoverySource, PackageDiscoveryService};

        let state = &mut self.state;
        state.popup_type = PackagePopupType::Import;

        // Get available package managers from PackageService, then convert to DiscoverySources
        // This filters out managers that don't support discovery (e.g., Custom, language managers)
        let available_managers = PackageService::get_available_managers();
        state.import_available_sources = available_managers
            .iter()
            .filter_map(DiscoverySource::from_package_manager)
            .collect();

        if state.import_available_sources.is_empty() {
            // No package managers that support discovery - show empty state
            state.import_loading = false;
            return Ok(());
        }

        // Reset selection and filter
        state.import_selected.clear();
        state.import_filter.clear();
        state.import_active_tab = 0;
        state.import_focus = ImportFocus::Filter; // Start with filter focused
        state.import_spinner_tick = 0;

        // Check cache for first source
        const CACHE_DURATION_SECS: u64 = 300;
        if state.import_cache_valid(CACHE_DURATION_SECS) {
            state.import_loading = false;
            state.import_discovery_rx = None;

            // Restore selection from cache if available
            let source = state.import_available_sources[0];
            if let Some(cache) = state.import_source_cache.get(&source) {
                state.import_selected = cache.selected.clone();
            }

            if !state.import_current_packages().is_empty() {
                state.import_list_state.select(Some(0));
            }
        } else {
            // Start async discovery for first source
            let source = state.import_available_sources[0];
            state.import_loading = true;
            state.import_discovery_rx =
                Some(PackageDiscoveryService::discover_source_async(source));
        }

        Ok(())
    }

    /// Switch to a different import tab (package manager source)
    fn switch_import_tab(&mut self, new_tab: usize) {
        use crate::utils::PackageDiscoveryService;

        if new_tab >= self.state.import_available_sources.len() {
            return;
        }

        // Save selection for current tab
        if let Some(current_source) = self
            .state
            .import_available_sources
            .get(self.state.import_active_tab)
        {
            if let Some(cache) = self.state.import_source_cache.get_mut(current_source) {
                cache.selected = self.state.import_selected.clone();
            }
        }

        let state = &mut self.state;
        state.import_active_tab = new_tab;

        // Clear selection and filter for new tab view
        state.import_selected.clear();
        state.import_filter.clear(); // Reset filter to avoid confusing state
        state.import_list_state = ratatui::widgets::ListState::default();

        // Check cache for new source
        const CACHE_DURATION_SECS: u64 = 300;
        if state.import_cache_valid(CACHE_DURATION_SECS) {
            state.import_loading = false;
            state.import_discovery_rx = None;

            // Restore selection from cache if available
            let new_source = state.import_available_sources[new_tab];
            if let Some(cache) = state.import_source_cache.get(&new_source) {
                state.import_selected = cache.selected.clone();
            }

            if !state.import_current_packages().is_empty() {
                state.import_list_state.select(Some(0));
            }
        } else {
            // Start async discovery for new source
            let source = state.import_available_sources[new_tab];
            state.import_loading = true;
            state.import_spinner_tick = 0;
            state.import_discovery_rx =
                Some(PackageDiscoveryService::discover_source_async(source));
        }
    }

    /// Poll the async discovery receiver for updates (non-blocking)
    fn poll_import_discovery(&mut self) -> bool {
        use crate::ui::ImportSourceCache;
        use crate::utils::DiscoveryStatus;
        use std::sync::mpsc::TryRecvError;
        use std::time::Instant;

        let rx = match self.state.import_discovery_rx.as_ref() {
            Some(rx) => rx,
            None => return false,
        };

        match rx.try_recv() {
            Ok(status) => {
                match status {
                    DiscoveryStatus::Started(_source) => {
                        // Just started, keep waiting
                    }
                    DiscoveryStatus::Complete { source, packages } => {
                        // Filter out packages that are already added (by binary name)
                        let existing_binaries: std::collections::HashSet<String> = self
                            .state
                            .packages
                            .iter()
                            .map(|p| p.binary_name.to_lowercase())
                            .collect();

                        let filtered_packages: Vec<_> = packages
                            .into_iter()
                            .filter(|p| {
                                let binary = p
                                    .binary_name
                                    .as_ref()
                                    .unwrap_or(&p.package_name)
                                    .to_lowercase();
                                !existing_binaries.contains(&binary)
                            })
                            .collect();

                        // Store in per-source cache
                        self.state.import_source_cache.insert(
                            source,
                            ImportSourceCache {
                                packages: filtered_packages,
                                discovered_at: Instant::now(),
                                selected: std::collections::HashSet::new(),
                            },
                        );

                        // Update list state
                        if !self.state.import_current_packages().is_empty() {
                            self.state.import_list_state.select(Some(0));
                        }

                        self.state.import_loading = false;
                        self.state.import_discovery_rx = None;
                    }
                    DiscoveryStatus::Failed { source: _, error } => {
                        warn!("Discovery failed: {}", error);
                        self.state.import_loading = false;
                        self.state.import_discovery_rx = None;
                    }
                    DiscoveryStatus::NoSourcesAvailable => {
                        self.state.import_loading = false;
                        self.state.import_discovery_rx = None;
                    }
                }
                true // Got a message
            }
            Err(TryRecvError::Empty) => false, // No message yet
            Err(TryRecvError::Disconnected) => {
                // Thread finished without sending Complete
                self.state.import_loading = false;
                self.state.import_discovery_rx = None;
                true
            }
        }
    }

    fn handle_add_edit_popup_event(
        &mut self,
        key: crossterm::event::KeyEvent,
        config: &Config,
    ) -> Result<ScreenAction> {
        let action = config.keymap.get_action(key.code, key.modifiers);
        let state = &mut self.state;

        // For plain character keys (no modifiers), ALWAYS insert the character
        // This ensures vim bindings like h/l don't interfere with typing
        if let KeyCode::Char(c) = key.code {
            if !key
                .modifiers
                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER)
            {
                match state.add_focused_field {
                    AddPackageField::Name => {
                        state.add_name_input.insert_char(c);
                    }
                    AddPackageField::Description => {
                        state.add_description_input.insert_char(c);
                    }
                    AddPackageField::PackageName => {
                        // Before inserting the new character, check if binary name should be auto-updated
                        // Get the current suggestion (before the new char)
                        let old_suggestion = PackageManagerImpl::suggest_binary_name(
                            state.add_package_name_input.text(),
                        );
                        let should_auto_update = state.add_binary_name_input.text().is_empty()
                            || state.add_binary_name_input.text() == old_suggestion;

                        state.add_package_name_input.insert_char(c);

                        // Update binary name suggestion if user hasn't manually edited it
                        if should_auto_update {
                            let new_suggestion = PackageManagerImpl::suggest_binary_name(
                                state.add_package_name_input.text(),
                            );
                            state.add_binary_name_input =
                                crate::utils::TextInput::with_text(new_suggestion);
                        }
                    }
                    AddPackageField::BinaryName => {
                        state.add_binary_name_input.insert_char(c);
                    }
                    AddPackageField::InstallCommand => {
                        state.add_install_command_input.insert_char(c);
                    }
                    AddPackageField::ExistenceCheck => {
                        state.add_existence_check_input.insert_char(c);
                    }
                    AddPackageField::ManagerCheck => {
                        state.add_manager_check_input.insert_char(c);
                    }
                    _ => {}
                }
                return Ok(ScreenAction::Refresh);
            }
        }

        // Handle actions for non-character keys (arrows, Tab, Esc, etc.)
        if let Some(action) = action {
            match action {
                Action::Cancel => {
                    self.reset_state();
                    return Ok(ScreenAction::Refresh);
                }
                Action::NextTab => {
                    state.add_focused_field = match state.add_focused_field {
                        AddPackageField::Name => AddPackageField::Description,
                        AddPackageField::Description => AddPackageField::Manager,
                        AddPackageField::Manager => {
                            if state.add_is_custom {
                                AddPackageField::BinaryName
                            } else {
                                AddPackageField::PackageName
                            }
                        }
                        AddPackageField::PackageName => AddPackageField::BinaryName,
                        AddPackageField::BinaryName => {
                            if state.add_is_custom {
                                AddPackageField::InstallCommand
                            } else {
                                AddPackageField::Name
                            }
                        }
                        AddPackageField::InstallCommand => AddPackageField::ExistenceCheck,
                        AddPackageField::ExistenceCheck => AddPackageField::Name,
                        AddPackageField::ManagerCheck => AddPackageField::Name,
                    };
                    return Ok(ScreenAction::Refresh);
                }
                Action::PrevTab => {
                    state.add_focused_field = match state.add_focused_field {
                        AddPackageField::Name => {
                            if state.add_is_custom {
                                AddPackageField::ExistenceCheck
                            } else {
                                AddPackageField::BinaryName
                            }
                        }
                        AddPackageField::Description => AddPackageField::Name,
                        AddPackageField::Manager => AddPackageField::Description,
                        AddPackageField::PackageName => AddPackageField::Manager,
                        AddPackageField::BinaryName => {
                            if state.add_is_custom {
                                AddPackageField::Manager
                            } else {
                                AddPackageField::PackageName
                            }
                        }
                        AddPackageField::InstallCommand => AddPackageField::BinaryName,
                        AddPackageField::ExistenceCheck => AddPackageField::InstallCommand,
                        AddPackageField::ManagerCheck => {
                            if state.add_is_custom {
                                AddPackageField::ExistenceCheck
                            } else {
                                AddPackageField::BinaryName
                            }
                        }
                    };
                    return Ok(ScreenAction::Refresh);
                }
                Action::Confirm => {
                    if state.add_focused_field == AddPackageField::Manager {
                        // Select manager
                        if !state.available_managers.is_empty() {
                            state.add_manager =
                                Some(state.available_managers[state.add_manager_selected].clone());
                            state.add_is_custom = matches!(
                                state.available_managers[state.add_manager_selected],
                                crate::utils::profile_manifest::PackageManager::Custom
                            );
                        }
                    } else {
                        // Save
                        let (
                            name,
                            description,
                            package_name,
                            binary_name,
                            install_command,
                            existence_check,
                            manager_check,
                            manager,
                            is_custom,
                            edit_idx,
                        ) = (
                            state.add_name_input.text().to_string(),
                            state.add_description_input.text().to_string(),
                            state.add_package_name_input.text().to_string(),
                            state.add_binary_name_input.text().to_string(),
                            state.add_install_command_input.text().to_string(),
                            state.add_existence_check_input.text().to_string(),
                            state.add_manager_check_input.text().to_string(),
                            state.add_manager.clone(),
                            state.add_is_custom,
                            state.add_editing_index,
                        );

                        // Validate
                        let validation = PackageService::validate_package(
                            &name,
                            &binary_name,
                            is_custom,
                            &package_name,
                            &install_command,
                            manager.as_ref(),
                        );

                        if !validation.is_valid {
                            warn!("Package validation failed: {:?}", validation.error_message);
                            // Set validation error to display inline in the popup
                            self.state.add_validation_error = validation.error_message;
                            return Ok(ScreenAction::Refresh);
                        }

                        // Check for duplicate binary name
                        let binary_name_lower = binary_name.trim().to_lowercase();
                        let duplicate = self.state.packages.iter().enumerate().any(|(idx, pkg)| {
                            // Skip the package being edited
                            if edit_idx == Some(idx) {
                                return false;
                            }
                            pkg.binary_name.to_lowercase() == binary_name_lower
                        });

                        if duplicate {
                            warn!(
                                "Package validation failed: duplicate binary name '{}'",
                                binary_name
                            );
                            self.state.add_validation_error = Some(format!(
                                "A package with binary '{}' already exists",
                                binary_name.trim()
                            ));
                            return Ok(ScreenAction::Refresh);
                        }

                        let manager = manager.unwrap(); // Validated
                        let package = PackageService::create_package(PackageCreationParams {
                            name: &name,
                            description: &description,
                            manager,
                            is_custom,
                            package_name: &package_name,
                            binary_name: &binary_name,
                            install_command: &install_command,
                            existence_check: &existence_check,
                            manager_check: &manager_check,
                        });

                        // Save
                        let repo_path = &config.repo_path;
                        let active_profile = &config.active_profile;
                        let is_new_package = edit_idx.is_none();
                        let packages = if let Some(idx) = edit_idx {
                            PackageService::update_package(repo_path, active_profile, idx, package)?
                        } else {
                            PackageService::add_package(repo_path, active_profile, package)?
                        };

                        // Track newly added package to prompt install after check
                        let new_package_index = if is_new_package {
                            Some(packages.len() - 1)
                        } else {
                            None
                        };

                        self.update_packages(packages, active_profile);

                        // Force newly added package to Unknown status so it gets checked
                        // (update_packages may have loaded a stale status from cache)
                        if let Some(idx) = new_package_index {
                            if idx < self.state.package_statuses.len() {
                                self.state.package_statuses[idx] = PackageStatus::Unknown;
                            }
                            self.state.newly_added_index = Some(idx);
                        }

                        self.reset_state();
                        // Trigger check for the new/updated package
                        self.state.is_checking = true;
                        return Ok(ScreenAction::Refresh);
                    }
                    return Ok(ScreenAction::Refresh);
                }
                Action::MoveUp | Action::MoveDown | Action::MoveLeft | Action::MoveRight => {
                    if state.add_focused_field == AddPackageField::Manager {
                        let count = state.available_managers.len();
                        if count > 0 {
                            if matches!(action, Action::MoveDown | Action::MoveRight) {
                                state.add_manager_selected =
                                    (state.add_manager_selected + 1) % count;
                            } else {
                                state.add_manager_selected = if state.add_manager_selected == 0 {
                                    count - 1
                                } else {
                                    state.add_manager_selected - 1
                                };
                            }
                            state.add_manager =
                                Some(state.available_managers[state.add_manager_selected].clone());
                            state.add_is_custom = matches!(
                                state.available_managers[state.add_manager_selected],
                                crate::utils::profile_manifest::PackageManager::Custom
                            );
                            state
                                .manager_list_state
                                .select(Some(state.add_manager_selected));
                        }
                        return Ok(ScreenAction::Refresh);
                    }
                }
                Action::Home | Action::End | Action::Backspace | Action::DeleteChar => {
                    // Handled below with text input helpers
                }
                _ => {}
            }
        }

        // Text input handling - cursor movement
        match action {
            Some(Action::MoveLeft) => {
                match state.add_focused_field {
                    AddPackageField::Name => state.add_name_input.move_left(),
                    AddPackageField::Description => state.add_description_input.move_left(),
                    AddPackageField::PackageName => state.add_package_name_input.move_left(),
                    AddPackageField::BinaryName => state.add_binary_name_input.move_left(),
                    AddPackageField::InstallCommand => state.add_install_command_input.move_left(),
                    AddPackageField::ExistenceCheck => state.add_existence_check_input.move_left(),
                    _ => {}
                }
                return Ok(ScreenAction::Refresh);
            }
            Some(Action::MoveRight) => {
                match state.add_focused_field {
                    AddPackageField::Name => state.add_name_input.move_right(),
                    AddPackageField::Description => state.add_description_input.move_right(),
                    AddPackageField::PackageName => state.add_package_name_input.move_right(),
                    AddPackageField::BinaryName => state.add_binary_name_input.move_right(),
                    AddPackageField::InstallCommand => state.add_install_command_input.move_right(),
                    AddPackageField::ExistenceCheck => state.add_existence_check_input.move_right(),
                    _ => {}
                }
                return Ok(ScreenAction::Refresh);
            }
            Some(Action::Home) => {
                match state.add_focused_field {
                    AddPackageField::Name => state.add_name_input.move_home(),
                    AddPackageField::Description => state.add_description_input.move_home(),
                    AddPackageField::PackageName => state.add_package_name_input.move_home(),
                    AddPackageField::BinaryName => state.add_binary_name_input.move_home(),
                    AddPackageField::InstallCommand => state.add_install_command_input.move_home(),
                    AddPackageField::ExistenceCheck => state.add_existence_check_input.move_home(),
                    _ => {}
                }
                return Ok(ScreenAction::Refresh);
            }
            Some(Action::End) => {
                match state.add_focused_field {
                    AddPackageField::Name => state.add_name_input.move_end(),
                    AddPackageField::Description => state.add_description_input.move_end(),
                    AddPackageField::PackageName => state.add_package_name_input.move_end(),
                    AddPackageField::BinaryName => state.add_binary_name_input.move_end(),
                    AddPackageField::InstallCommand => state.add_install_command_input.move_end(),
                    AddPackageField::ExistenceCheck => state.add_existence_check_input.move_end(),
                    _ => {}
                }
                return Ok(ScreenAction::Refresh);
            }
            _ => {}
        }

        if let Some(Action::Backspace) = action {
            match state.add_focused_field {
                AddPackageField::Name => {
                    state.add_name_input.backspace();
                }
                AddPackageField::Description => {
                    state.add_description_input.backspace();
                }
                AddPackageField::PackageName => {
                    // Before backspacing, check if binary name should be auto-updated
                    let old_suggestion = PackageManagerImpl::suggest_binary_name(
                        state.add_package_name_input.text(),
                    );
                    let should_auto_update = state.add_binary_name_input.text().is_empty()
                        || state.add_binary_name_input.text() == old_suggestion;

                    state.add_package_name_input.backspace();

                    // Update binary name suggestion if user hasn't manually edited it
                    if should_auto_update {
                        let new_suggestion = PackageManagerImpl::suggest_binary_name(
                            state.add_package_name_input.text(),
                        );
                        state.add_binary_name_input =
                            crate::utils::TextInput::with_text(new_suggestion);
                    }
                }
                AddPackageField::BinaryName => {
                    state.add_binary_name_input.backspace();
                }
                AddPackageField::InstallCommand => {
                    state.add_install_command_input.backspace();
                }
                AddPackageField::ExistenceCheck => {
                    state.add_existence_check_input.backspace();
                }
                _ => {}
            }
            return Ok(ScreenAction::Refresh);
        }

        if let Some(Action::DeleteChar) = action {
            match state.add_focused_field {
                AddPackageField::Name => {
                    state.add_name_input.delete();
                }
                AddPackageField::Description => {
                    state.add_description_input.delete();
                }
                AddPackageField::PackageName => {
                    // Before deleting, check if binary name should be auto-updated
                    let old_suggestion = PackageManagerImpl::suggest_binary_name(
                        state.add_package_name_input.text(),
                    );
                    let should_auto_update = state.add_binary_name_input.text().is_empty()
                        || state.add_binary_name_input.text() == old_suggestion;

                    state.add_package_name_input.delete();

                    // Update binary name suggestion if user hasn't manually edited it
                    if should_auto_update {
                        let new_suggestion = PackageManagerImpl::suggest_binary_name(
                            state.add_package_name_input.text(),
                        );
                        state.add_binary_name_input =
                            crate::utils::TextInput::with_text(new_suggestion);
                    }
                }
                AddPackageField::BinaryName => {
                    state.add_binary_name_input.delete();
                }
                AddPackageField::InstallCommand => {
                    state.add_install_command_input.delete();
                }
                AddPackageField::ExistenceCheck => {
                    state.add_existence_check_input.delete();
                }
                _ => {}
            }
            return Ok(ScreenAction::Refresh);
        }

        // Char input
        if let KeyCode::Char(c) = key.code {
            if !key
                .modifiers
                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER)
            {
                // Clear validation error when user starts typing
                state.add_validation_error = None;

                match state.add_focused_field {
                    AddPackageField::Name => {
                        state.add_name_input.insert_char(c);
                    }
                    AddPackageField::Description => {
                        state.add_description_input.insert_char(c);
                    }
                    AddPackageField::PackageName => {
                        // Before inserting the new character, check if binary name should be auto-updated
                        // Get the current suggestion (before the new char)
                        let old_suggestion = PackageManagerImpl::suggest_binary_name(
                            state.add_package_name_input.text(),
                        );
                        let should_auto_update = state.add_binary_name_input.text().is_empty()
                            || state.add_binary_name_input.text() == old_suggestion;

                        state.add_package_name_input.insert_char(c);

                        // Update binary name suggestion if user hasn't manually edited it
                        if should_auto_update {
                            let new_suggestion = PackageManagerImpl::suggest_binary_name(
                                state.add_package_name_input.text(),
                            );
                            state.add_binary_name_input =
                                crate::utils::TextInput::with_text(new_suggestion);
                        }
                    }
                    AddPackageField::BinaryName => {
                        state.add_binary_name_input.insert_char(c);
                    }
                    AddPackageField::InstallCommand => {
                        state.add_install_command_input.insert_char(c);
                    }
                    AddPackageField::ExistenceCheck => {
                        state.add_existence_check_input.insert_char(c);
                    }
                    _ => {}
                }
                return Ok(ScreenAction::Refresh);
            }
        }

        Ok(ScreenAction::None)
    }

    fn handle_delete_popup_event(
        &mut self,
        key: crossterm::event::KeyEvent,
        config: &Config,
    ) -> Result<ScreenAction> {
        let action = config.keymap.get_action(key.code, key.modifiers);
        let state = &mut self.state;

        if let Some(action) = action {
            match action {
                Action::Cancel => {
                    self.reset_state();
                    return Ok(ScreenAction::Refresh);
                }
                Action::Confirm => {
                    if state.delete_confirm_input.text().trim() == "DELETE" {
                        if let Some(idx) = state.delete_index {
                            // Get package name before deletion to remove from cache
                            let package_name = state.packages.get(idx).map(|p| p.name.clone());

                            let packages = PackageService::delete_package(
                                &config.repo_path,
                                &config.active_profile,
                                idx,
                            )?;

                            // Remove from cache
                            if let Some(name) = package_name {
                                if let Err(e) = self
                                    .state
                                    .cache
                                    .remove_status(&config.active_profile, &name)
                                {
                                    warn!("Failed to remove package from cache: {}", e);
                                }
                            }

                            self.update_packages(packages, &config.active_profile);
                            self.reset_state();
                            return Ok(ScreenAction::Refresh);
                        }
                    }
                }
                Action::Backspace => {
                    state.delete_confirm_input.backspace();
                    return Ok(ScreenAction::Refresh);
                }
                Action::DeleteChar => {
                    state.delete_confirm_input.delete();
                    return Ok(ScreenAction::Refresh);
                }
                Action::MoveLeft => {
                    state.delete_confirm_input.move_left();
                    return Ok(ScreenAction::Refresh);
                }
                Action::MoveRight => {
                    state.delete_confirm_input.move_right();
                    return Ok(ScreenAction::Refresh);
                }
                Action::Home => {
                    state.delete_confirm_input.move_home();
                    return Ok(ScreenAction::Refresh);
                }
                Action::End => {
                    state.delete_confirm_input.move_end();
                    return Ok(ScreenAction::Refresh);
                }
                _ => {}
            }
        }

        if let KeyCode::Char(c) = key.code {
            if !key
                .modifiers
                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER)
            {
                state.delete_confirm_input.insert_char(c);
                return Ok(ScreenAction::Refresh);
            }
        }

        Ok(ScreenAction::None)
    }

    fn handle_import_popup_event(
        &mut self,
        key: crossterm::event::KeyEvent,
        config: &Config,
    ) -> Result<ScreenAction> {
        use crate::ui::ImportFocus;

        let action = config.keymap.get_action(key.code, key.modifiers);

        // Global actions (work regardless of focus)
        if let Some(action) = action {
            match action {
                Action::Cancel | Action::Quit => {
                    self.state.popup_type = PackagePopupType::None;
                    self.state.import_selected.clear();
                    self.state.import_filter.clear();
                    return Ok(ScreenAction::Refresh);
                }
                Action::Confirm => {
                    if !self.state.import_selected.is_empty() {
                        return self.import_selected_packages(config);
                    }
                }
                Action::SelectAll => {
                    let filtered = self.get_filtered_import_packages();
                    for &idx in &filtered {
                        self.state.import_selected.insert(idx);
                    }
                    return Ok(ScreenAction::Refresh);
                }
                Action::DeselectAll => {
                    self.state.import_selected.clear();
                    return Ok(ScreenAction::Refresh);
                }
                _ => {}
            }
        }

        // Focus cycling with Tab/Shift+Tab
        if key.code == KeyCode::Tab && !key.modifiers.contains(KeyModifiers::SHIFT) {
            // Only cycle focus if we have tabs to show
            if self.state.import_available_sources.len() > 1 {
                self.state.import_focus = match self.state.import_focus {
                    ImportFocus::Tabs => ImportFocus::Filter,
                    ImportFocus::Filter => ImportFocus::List,
                    ImportFocus::List => ImportFocus::Tabs,
                };
            } else {
                // No tabs or only one source - cycle between Filter and List
                self.state.import_focus = match self.state.import_focus {
                    ImportFocus::Tabs => ImportFocus::Filter,
                    ImportFocus::Filter => ImportFocus::List,
                    ImportFocus::List => ImportFocus::Filter,
                };
            }
            return Ok(ScreenAction::Refresh);
        }
        if key.code == KeyCode::BackTab {
            if self.state.import_available_sources.len() > 1 {
                self.state.import_focus = match self.state.import_focus {
                    ImportFocus::Tabs => ImportFocus::List,
                    ImportFocus::Filter => ImportFocus::Tabs,
                    ImportFocus::List => ImportFocus::Filter,
                };
            } else {
                self.state.import_focus = match self.state.import_focus {
                    ImportFocus::Tabs => ImportFocus::List,
                    ImportFocus::Filter => ImportFocus::List,
                    ImportFocus::List => ImportFocus::Filter,
                };
            }
            return Ok(ScreenAction::Refresh);
        }

        // Focus-specific handling
        match self.state.import_focus {
            ImportFocus::Tabs => self.handle_import_tabs_input(key, config),
            ImportFocus::Filter => self.handle_import_filter_input(key, config),
            ImportFocus::List => self.handle_import_list_input(key, config),
        }
    }

    fn handle_import_tabs_input(
        &mut self,
        key: crossterm::event::KeyEvent,
        config: &Config,
    ) -> Result<ScreenAction> {
        use crate::ui::ImportFocus;

        let action = config.keymap.get_action(key.code, key.modifiers);

        if let Some(action) = action {
            match action {
                Action::MoveLeft => {
                    if self.state.import_active_tab > 0 {
                        self.switch_import_tab(self.state.import_active_tab - 1);
                    }
                    return Ok(ScreenAction::Refresh);
                }
                Action::MoveRight => {
                    if self.state.import_active_tab
                        < self.state.import_available_sources.len().saturating_sub(1)
                    {
                        self.switch_import_tab(self.state.import_active_tab + 1);
                    }
                    return Ok(ScreenAction::Refresh);
                }
                Action::MoveDown => {
                    // Move focus to filter
                    self.state.import_focus = ImportFocus::Filter;
                    return Ok(ScreenAction::Refresh);
                }
                _ => {}
            }
        }
        Ok(ScreenAction::None)
    }

    fn handle_import_filter_input(
        &mut self,
        key: crossterm::event::KeyEvent,
        config: &Config,
    ) -> Result<ScreenAction> {
        use crate::ui::ImportFocus;

        let action = config.keymap.get_action(key.code, key.modifiers);

        // Handle backspace
        if let Some(Action::Backspace) = action {
            self.state.import_filter.backspace();
            self.reset_import_list_selection();
            return Ok(ScreenAction::Refresh);
        }

        // Handle character input (exclude Space which is for ToggleSelect)
        if let KeyCode::Char(c) = key.code {
            if !key
                .modifiers
                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER)
                && c != ' '
            {
                self.state.import_filter.insert_char(c);
                self.reset_import_list_selection();
                return Ok(ScreenAction::Refresh);
            }
        }

        // Space toggles selection (even when filter focused)
        if let Some(Action::ToggleSelect) = action {
            self.toggle_import_selection();
            return Ok(ScreenAction::Refresh);
        }

        // Navigation moves focus
        if let Some(action) = action {
            match action {
                Action::MoveUp => {
                    if self.state.import_available_sources.len() > 1 {
                        self.state.import_focus = ImportFocus::Tabs;
                    }
                    return Ok(ScreenAction::Refresh);
                }
                Action::MoveDown => {
                    self.state.import_focus = ImportFocus::List;
                    return Ok(ScreenAction::Refresh);
                }
                _ => {}
            }
        }

        Ok(ScreenAction::None)
    }

    fn handle_import_list_input(
        &mut self,
        key: crossterm::event::KeyEvent,
        config: &Config,
    ) -> Result<ScreenAction> {
        let action = config.keymap.get_action(key.code, key.modifiers);

        if let Some(action) = action {
            match action {
                Action::MoveUp => {
                    let filtered = self.get_filtered_import_packages();
                    if !filtered.is_empty() {
                        let current = self.state.import_list_state.selected().unwrap_or(0);
                        let new_idx = if current == 0 {
                            filtered.len() - 1
                        } else {
                            current - 1
                        };
                        self.state.import_list_state.select(Some(new_idx));
                    }
                    return Ok(ScreenAction::Refresh);
                }
                Action::MoveDown => {
                    let filtered = self.get_filtered_import_packages();
                    if !filtered.is_empty() {
                        let current = self.state.import_list_state.selected().unwrap_or(0);
                        let new_idx = (current + 1) % filtered.len();
                        self.state.import_list_state.select(Some(new_idx));
                    }
                    return Ok(ScreenAction::Refresh);
                }
                Action::ToggleSelect => {
                    self.toggle_import_selection();
                    return Ok(ScreenAction::Refresh);
                }
                _ => {}
            }
        }

        Ok(ScreenAction::None)
    }

    fn toggle_import_selection(&mut self) {
        let filtered = self.get_filtered_import_packages();
        if let Some(filtered_idx) = self.state.import_list_state.selected() {
            if let Some(&original_idx) = filtered.get(filtered_idx) {
                if self.state.import_selected.contains(&original_idx) {
                    self.state.import_selected.remove(&original_idx);
                } else {
                    self.state.import_selected.insert(original_idx);
                }
            }
        }
    }

    fn reset_import_list_selection(&mut self) {
        let filtered = self.get_filtered_import_packages();
        if filtered.is_empty() {
            self.state.import_list_state.select(None);
        } else {
            self.state.import_list_state.select(Some(0));
        }
    }

    fn get_filtered_import_packages(&self) -> Vec<usize> {
        let filter = self.state.import_filter.text().to_lowercase();
        let packages = self.state.import_current_packages();

        packages
            .iter()
            .enumerate()
            .filter(|(_, pkg)| {
                if filter.is_empty() {
                    return true;
                }
                pkg.package_name.to_lowercase().contains(&filter)
                    || pkg
                        .binary_name
                        .as_ref()
                        .is_some_and(|b| b.to_lowercase().contains(&filter))
            })
            .map(|(idx, _)| idx)
            .collect()
    }

    fn import_selected_packages(&mut self, config: &Config) -> Result<ScreenAction> {
        // First, sync current tab's selection to cache so it's included in the loop
        if let Some(current_source) = self
            .state
            .import_available_sources
            .get(self.state.import_active_tab)
        {
            if let Some(cache) = self.state.import_source_cache.get_mut(current_source) {
                cache.selected = self.state.import_selected.clone();
            }
        }

        // Collect all packages to import first to avoid borrow issues
        let mut all_packages_to_import = Vec::new();

        for (source, cache) in &self.state.import_source_cache {
            if cache.selected.is_empty() {
                continue;
            }

            let manager = source.to_package_manager();

            let packages: Vec<crate::utils::DiscoveredPackage> = cache
                .selected
                .iter()
                .filter_map(|&idx| cache.packages.get(idx).cloned())
                .collect();

            for pkg in packages {
                all_packages_to_import.push((manager.clone(), pkg));
            }
        }

        let mut packages_imported = false;

        for (manager, discovered) in all_packages_to_import {
            let binary_name = discovered
                .binary_name
                .clone()
                .unwrap_or_else(|| discovered.package_name.clone());

            let package = PackageService::create_package(PackageCreationParams {
                name: &discovered.package_name,
                description: &discovered.description.clone().unwrap_or_default(),
                manager: manager.clone(),
                is_custom: false,
                package_name: &discovered.package_name,
                binary_name: &binary_name,
                install_command: "",
                existence_check: "",
                manager_check: "",
            });

            match PackageService::add_package(&config.repo_path, &config.active_profile, package) {
                Ok(packages) => {
                    self.update_packages(packages, &config.active_profile);
                    packages_imported = true;
                }
                Err(e) => {
                    warn!(
                        "Failed to import package {}: {}",
                        discovered.package_name, e
                    );
                }
            }
        }

        if !packages_imported {
            return Ok(ScreenAction::Refresh);
        }

        // Clear all selections after import
        for cache in self.state.import_source_cache.values_mut() {
            cache.packages.retain(|_p| {
                // Determine if we should keep it. Ideally we remove imported ones.
                // But complex to track which exact ones succeeded.
                // For simplicity, just clearing selection is enough UX.
                // The next discovery will filter them out anyway.
                true
            });
            cache.selected.clear();
        }

        // Close popup and trigger check
        self.state.popup_type = PackagePopupType::None;
        self.state.import_selected.clear();
        self.state.import_filter.clear();
        self.state.is_checking = true;

        Ok(ScreenAction::Refresh)
    }
}

// Rendering methods inlined from PackageManagerComponent
impl ManagePackagesScreen {
    fn render_package_list(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        config: &Config,
    ) -> Result<()> {
        let t = theme();

        if self.state.packages.is_empty() {
            // Show empty state message
            let paragraph = Paragraph::new(format!(
                "No packages yet.\n\nPress '{}' to add your first package.",
                config
                    .keymap
                    .get_key_display_for_action(crate::keymap::Action::Create)
            ))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(theme().border_type(false))
                    .title(" Packages ")
                    .border_style(unfocused_border_style())
                    .padding(Padding::uniform(1)),
            )
            .wrap(Wrap { trim: true })
            .alignment(Alignment::Center);
            frame.render_widget(paragraph, area);
        } else {
            let items: Vec<ListItem> = self
                .state
                .packages
                .iter()
                .enumerate()
                .map(|(idx, package)| {
                    let icons = crate::icons::Icons::from_config(config);
                    let status_icon = match self.state.package_statuses.get(idx) {
                        Some(PackageStatus::Installed) => icons.success(),
                        Some(PackageStatus::NotInstalled) => icons.error(),
                        Some(PackageStatus::Error(_)) => icons.warning(),
                        _ => {
                            if self.state.is_checking && self.state.checking_index == Some(idx) {
                                icons.loading()
                            } else {
                                " "
                            }
                        }
                    };

                    let manager_str = format!("{:?}", package.manager);
                    let manager_len = manager_str.chars().count();

                    // Approximate widths (assume ascii mostly)
                    let status_char_count = status_icon.chars().count();
                    let name_char_count = package.name.chars().count();

                    // Available width: Area width - borders (2) - highlight symbol (2)
                    let inner_width = area.width.saturating_sub(4) as usize;

                    // Calculate padding
                    let used_width = status_char_count + 1 + name_char_count + 1 + manager_len; // +1s for spaces
                    let padding_len = inner_width.saturating_sub(used_width);
                    let padding = " ".repeat(padding_len);

                    let style = match self.state.package_statuses.get(idx) {
                        Some(PackageStatus::Installed) => Style::default().fg(t.success),
                        Some(PackageStatus::NotInstalled) => Style::default().fg(t.error),
                        Some(PackageStatus::Error(_)) => Style::default().fg(t.warning),
                        _ => Style::default(),
                    };

                    let line = Line::from(vec![
                        Span::styled(status_icon, style),
                        Span::styled(" ", style),
                        Span::styled(&package.name, style),
                        Span::raw(padding),
                        Span::styled(
                            format!(" {manager_str}"),
                            Style::default().italic().fg(t.text_dimmed),
                        ),
                    ]);

                    ListItem::new(line)
                })
                .collect();

            let list = List::new(items)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_type(theme().border_type(false))
                        .title(" Packages ")
                        .border_style(focused_border_style()),
                )
                .highlight_style(t.highlight_style())
                .highlight_symbol(LIST_HIGHLIGHT_SYMBOL);

            frame.render_stateful_widget(list, area, &mut self.state.list_state);

            // Populate mouse regions
            self.list_pane_area = Some(area);
            self.mouse_regions.clear();
            let inner = Block::default().borders(Borders::ALL).inner(area);
            let scroll_offset = self.state.list_state.offset();
            for i in 0..self.state.packages.len() {
                if i < scroll_offset {
                    continue;
                }
                let visible_row = (i - scroll_offset) as u16;
                if visible_row >= inner.height {
                    break;
                }
                let row_area = Rect::new(inner.x, inner.y + visible_row, inner.width, 1);
                self.mouse_regions.add(row_area, i);
            }
        }

        Ok(())
    }

    fn render_package_details(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        config: &Config,
    ) -> Result<()> {
        let selected = self.state.list_state.selected();
        let details = if let Some(idx) = selected {
            if let Some(package) = self.state.packages.get(idx) {
                self.format_package_details(package, idx, config)
            } else {
                vec![Line::from("No package selected")]
            }
        } else {
            vec![Line::from("No package selected")]
        };

        let paragraph = Paragraph::new(details)
            .style(theme().text_style())
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(theme().border_type(false))
                    .padding(Padding::uniform(1))
                    .title(" Package Details ")
                    .style(theme().background_style())
                    .title_style(theme().title_style()),
            )
            .wrap(Wrap { trim: true });

        frame.render_widget(paragraph, area);

        Ok(())
    }

    fn format_package_details(
        &self,
        package: &Package,
        idx: usize,
        config: &Config,
    ) -> Vec<Line<'_>> {
        let t = theme();
        let icons = crate::icons::Icons::from_config(config);
        let mut lines = Vec::new();

        // Helper for labeled fields
        let add_field = |lines: &mut Vec<Line>, label: &str, value: &str| {
            lines.push(Line::from(vec![
                Span::styled(format!("{label}: "), t.title_style()),
                Span::styled(value.to_string(), t.text_style()),
            ]));
        };

        add_field(&mut lines, "Name", &package.name);

        if let Some(desc) = &package.description {
            lines.push(Line::from(""));
            lines.push(Line::from(vec![Span::styled(
                "Description:",
                t.title_style(),
            )]));
            lines.push(Line::from(vec![Span::styled(
                desc.clone(),
                t.muted_style(),
            )]));
        }

        lines.push(Line::from(""));
        add_field(&mut lines, "Manager", &format!("{:?}", package.manager));

        if let Some(pkg_name) = &package.package_name {
            add_field(&mut lines, "Package Name", pkg_name);
        }

        add_field(&mut lines, "Binary Name", &package.binary_name);

        // Status
        lines.push(Line::from(""));
        let status = self.state.package_statuses.get(idx);
        match status {
            Some(PackageStatus::Installed) => {
                lines.push(Line::from(vec![
                    Span::styled("Status: ", t.title_style()),
                    Span::styled(format!("{} Installed", icons.success()), t.success_style()),
                ]));
            }
            Some(PackageStatus::NotInstalled) => {
                lines.push(Line::from(vec![
                    Span::styled("Status: ", t.title_style()),
                    Span::styled(format!("{} Not Installed", icons.error()), t.error_style()),
                ]));

                // Check if manager is installed for installation purposes
                if !PackageManagerImpl::is_manager_installed(&package.manager) {
                    lines.push(Line::from(""));
                    lines.push(Line::from(vec![Span::styled(
                        format!(
                            "{} Package manager '{:?}' is not installed",
                            icons.warning(),
                            package.manager
                        ),
                        t.warning_style(),
                    )]));

                    lines.push(Line::from(""));
                    lines.push(Line::from(vec![Span::styled(
                        "Installation instructions:",
                        t.title_style(),
                    )]));

                    let instructions =
                        PackageManagerImpl::installation_instructions(&package.manager);
                    for line in instructions.lines() {
                        lines.push(Line::from(vec![Span::styled(
                            line.to_string(),
                            t.muted_style(),
                        )]));
                    }
                }
            }
            Some(PackageStatus::Error(msg)) => {
                lines.push(Line::from(vec![
                    Span::styled("Status: ", t.title_style()),
                    Span::styled(format!("{} Error: ", icons.warning()), t.warning_style()),
                    Span::styled(msg.clone(), t.text_style()),
                ]));
            }
            _ => {
                lines.push(Line::from(vec![
                    Span::styled("Status: ", t.title_style()),
                    Span::styled(format!("{} Unknown", icons.loading()), t.muted_style()),
                    Span::styled(" (press ", t.muted_style()),
                    Span::styled(
                        config
                            .keymap
                            .get_key_display_for_action(crate::keymap::Action::CheckStatus),
                        t.emphasis_style(),
                    ),
                    Span::styled(" to check)", t.muted_style()),
                ]));
            }
        }

        // Cache details
        if let Some(entry) = self
            .state
            .cache
            .get_status(&self.state.active_profile, &package.name)
        {
            lines.push(Line::from(""));
            lines.push(Line::from(""));
            lines.push(Line::from(vec![Span::styled(
                "-- Last Check Details --",
                t.muted_style(),
            )]));

            lines.push(Line::from(vec![
                Span::styled("Time: ", t.title_style()),
                Span::styled(
                    entry
                        .last_checked
                        .format("%Y-%m-%d %H:%M:%S UTC")
                        .to_string(),
                    t.text_style(),
                ),
            ]));

            if let Some(cmd) = &entry.check_command {
                lines.push(Line::from(vec![
                    Span::styled("Command: ", t.title_style()),
                    Span::styled(cmd.clone(), t.emphasis_style()),
                ]));
            }

            if let Some(output) = &entry.output {
                // Truncate output if too long
                let display_output = if output.len() > 500 {
                    format!("{}... (truncated)", &output[..500])
                } else {
                    output.clone()
                };

                lines.push(Line::from(vec![Span::styled("Output:", t.title_style())]));

                for line in display_output.lines() {
                    lines.push(Line::from(vec![Span::styled(
                        line.to_string(),
                        t.muted_style(),
                    )]));
                }
            }
        }

        lines
    }

    fn render_popup(&mut self, frame: &mut Frame, area: Rect, config: &Config) -> Result<()> {
        match self.state.popup_type {
            PackagePopupType::Add | PackagePopupType::Edit => {
                self.render_add_edit_popup(frame, area, config)?;
            }
            PackagePopupType::Delete => {
                self.render_delete_popup(frame, area, config)?;
            }
            PackagePopupType::InstallMissing => {
                self.render_install_missing_popup(frame, area, config)?;
            }
            PackagePopupType::Import => {
                self.render_import_popup(frame, area, config)?;
            }
            PackagePopupType::None => return Ok(()),
        }
        Ok(())
    }

    fn render_add_edit_popup(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        config: &Config,
    ) -> Result<()> {
        use crate::components::Popup;

        let t = theme();
        // Make popup larger to fit all fields, especially for custom packages
        let popup_height = if self.state.add_is_custom { 60 } else { 50 };
        let result = Popup::new()
            .width(80)
            .height(popup_height)
            .dim_background(true)
            .render(frame, area);
        let popup_area = result.content_area;

        let title = if self.state.add_editing_index.is_some() {
            "Edit Package"
        } else {
            "Add Package"
        };

        // Build constraints dynamically based on package type
        let mut constraints = vec![
            Constraint::Length(1), // Title
            Constraint::Length(3), // Name
            Constraint::Length(3), // Description
            Constraint::Length(4), // Manager selection
        ];

        if self.state.add_is_custom {
            // Custom packages: Binary Name, Install Command, Existence Check
            constraints.push(Constraint::Length(3)); // Binary name
            constraints.push(Constraint::Length(3)); // Install command
            constraints.push(Constraint::Length(3)); // Existence check
        } else {
            // Managed packages: Package Name, Binary Name
            constraints.push(Constraint::Length(3)); // Package name
            constraints.push(Constraint::Length(3)); // Binary name
        }

        // Error message (if any)
        if self.state.add_validation_error.is_some() {
            constraints.push(Constraint::Length(2)); // Error message
        }
        constraints.push(Constraint::Min(0)); // Spacer
        constraints.push(Constraint::Length(2)); // Footer

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(constraints)
            .split(popup_area);

        // Store clickable field areas for mouse support
        self.add_field_areas.clear();

        // Title (no border, just text)
        let title_para = Paragraph::new(title)
            .alignment(Alignment::Center)
            .style(t.title_style());
        frame.render_widget(title_para, chunks[0]);

        // Name field
        self.add_field_areas
            .push((chunks[1], AddPackageField::Name));
        let widget = TextInputWidget::new(&self.state.add_name_input)
            .title("Name")
            .placeholder("Package display name")
            .focused(self.state.add_focused_field == AddPackageField::Name);
        frame.render_text_input_widget(widget, chunks[1]);

        // Description field
        self.add_field_areas
            .push((chunks[2], AddPackageField::Description));
        let widget = TextInputWidget::new(&self.state.add_description_input)
            .title("Description (optional)")
            .placeholder("Package description")
            .focused(self.state.add_focused_field == AddPackageField::Description);
        frame.render_text_input_widget(widget, chunks[2]);

        // Manager selection
        self.add_field_areas
            .push((chunks[3], AddPackageField::Manager));
        self.render_manager_selection(frame, chunks[3])?;

        let mut current_chunk = 4; // Start after title, name, description, manager

        if self.state.add_is_custom {
            // Custom packages: Binary Name, Install Command, Existence Check
            self.add_field_areas
                .push((chunks[current_chunk], AddPackageField::BinaryName));
            let widget = TextInputWidget::new(&self.state.add_binary_name_input)
                .title("Binary Name")
                .placeholder("Binary name to check (e.g., 'mytool')")
                .focused(self.state.add_focused_field == AddPackageField::BinaryName);
            frame.render_text_input_widget(widget, chunks[current_chunk]);
            current_chunk += 1;

            self.add_field_areas
                .push((chunks[current_chunk], AddPackageField::InstallCommand));
            let widget = TextInputWidget::new(&self.state.add_install_command_input)
                .title("Install Command")
                .placeholder("Install command (e.g., './install.sh')")
                .focused(self.state.add_focused_field == AddPackageField::InstallCommand);
            frame.render_text_input_widget(widget, chunks[current_chunk]);
            current_chunk += 1;

            self.add_field_areas
                .push((chunks[current_chunk], AddPackageField::ExistenceCheck));
            let widget = TextInputWidget::new(&self.state.add_existence_check_input)
                .title("Existence Check (optional)")
                .placeholder(
                    "Command to check if package exists (if empty, uses binary name check)",
                )
                .focused(self.state.add_focused_field == AddPackageField::ExistenceCheck);
            frame.render_text_input_widget(widget, chunks[current_chunk]);
            current_chunk += 1;

            self.add_field_areas
                .push((chunks[current_chunk], AddPackageField::ManagerCheck));
            let widget = TextInputWidget::new(&self.state.add_manager_check_input)
                .title("Manager Check (optional)")
                .placeholder("Custom manager check command (optional fallback)")
                .focused(self.state.add_focused_field == AddPackageField::ManagerCheck);
            frame.render_text_input_widget(widget, chunks[current_chunk]);
            current_chunk += 1;
        } else {
            // Managed packages: Package Name, Binary Name
            self.add_field_areas
                .push((chunks[current_chunk], AddPackageField::PackageName));
            let widget = TextInputWidget::new(&self.state.add_package_name_input)
                .title("Package Name")
                .placeholder("Package name in manager (e.g., 'eza')")
                .focused(self.state.add_focused_field == AddPackageField::PackageName);
            frame.render_text_input_widget(widget, chunks[current_chunk]);
            current_chunk += 1;

            self.add_field_areas
                .push((chunks[current_chunk], AddPackageField::BinaryName));
            let widget = TextInputWidget::new(&self.state.add_binary_name_input)
                .title("Binary Name")
                .placeholder("Binary name to check (e.g., 'eza')")
                .focused(self.state.add_focused_field == AddPackageField::BinaryName);
            frame.render_text_input_widget(widget, chunks[current_chunk]);
            current_chunk += 1;
        }

        // Validation error message (if any)
        if let Some(error) = &self.state.add_validation_error {
            let error_para = Paragraph::new(error.as_str())
                .style(Style::default().fg(t.error))
                .alignment(Alignment::Center);
            frame.render_widget(error_para, chunks[current_chunk]);
        }

        // Footer with instructions (always the last chunk)
        let k = |a| config.keymap.get_key_display_for_action(a);
        let footer_text = format!(
            "{}: Next field | {}: Previous | {}: Save | {}: Cancel",
            k(crate::keymap::Action::NextTab),
            k(crate::keymap::Action::PrevTab),
            k(crate::keymap::Action::Confirm),
            k(crate::keymap::Action::Cancel)
        );
        Footer::render(frame, chunks[chunks.len() - 1], &footer_text)?;

        Ok(())
    }

    fn render_manager_selection(&mut self, frame: &mut Frame, area: Rect) -> Result<()> {
        // Initialize available managers if empty
        if self.state.available_managers.is_empty() {
            self.state.available_managers = PackageManagerImpl::get_available_managers();
            if !self.state.available_managers.is_empty() {
                self.state.add_manager = Some(self.state.available_managers[0].clone());
                self.state.add_manager_selected = 0;
            }
        }

        // Create manager labels with selection state
        let manager_labels: Vec<(String, bool)> = self
            .state
            .available_managers
            .iter()
            .enumerate()
            .map(|(idx, manager)| {
                let is_selected = self.state.add_manager_selected == idx;
                let label = format!("{manager:?}");
                (label, is_selected)
            })
            .collect();

        // Render checkboxes in a horizontal wrapping layout
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(theme().border_type(false))
            .title(" Package Manager ")
            .border_style(
                if self.state.add_focused_field == AddPackageField::Manager {
                    focused_border_style()
                } else {
                    unfocused_border_style()
                },
            );

        let inner_area = block.inner(area);
        frame.render_widget(block, area);

        // Calculate how many checkboxes fit per row and render them
        let available_width = inner_area.width as usize;
        let mut current_x = 0;
        let mut current_y = 0;
        let line_height = 1;

        let t = theme();
        for (idx, (label, is_selected)) in manager_labels.iter().enumerate() {
            // Checkbox format: "[x] Label " or "[ ] Label "
            let checkbox_marker = if *is_selected { "[x]" } else { "[ ]" };
            let full_text = format!("{checkbox_marker} {label} ");
            let checkbox_width = full_text.len();

            // Check if we need to wrap to next line
            if current_x > 0 && (current_x + checkbox_width) > available_width {
                current_x = 0;
                current_y += line_height;
            }

            // Check if we have enough vertical space
            if current_y >= inner_area.height as usize {
                break; // Don't render if we're out of space
            }

            let checkbox_area = Rect::new(
                inner_area.x + current_x as u16,
                inner_area.y + current_y as u16,
                checkbox_width.min(available_width - current_x) as u16,
                line_height as u16,
            );

            // Create styled text for checkbox
            let is_focused = self.state.add_focused_field == AddPackageField::Manager
                && self.state.add_manager_selected == idx;
            let checkbox_style = if is_focused {
                Style::default()
                    .fg(t.text_emphasis)
                    .add_modifier(Modifier::BOLD)
            } else if *is_selected {
                Style::default().fg(t.success)
            } else {
                t.text_style()
            };

            let checkbox_text = Paragraph::new(full_text).style(checkbox_style);
            frame.render_widget(checkbox_text, checkbox_area);

            // Update selected manager if this checkbox is selected
            if *is_selected {
                self.state.add_manager = Some(self.state.available_managers[idx].clone());
                self.state.add_manager_selected = idx;
                // Auto-detect if custom
                self.state.add_is_custom =
                    matches!(self.state.available_managers[idx], PackageManager::Custom);
            }

            current_x += checkbox_width;
        }

        Ok(())
    }

    fn render_delete_popup(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        config: &Config,
    ) -> Result<()> {
        use crate::widgets::{Dialog, DialogVariant};

        let package_name = if let Some(idx) = self.state.delete_index {
            self.state
                .packages
                .get(idx)
                .map_or("Unknown", |p| p.name.as_str())
        } else {
            "Unknown"
        };

        let content = format!(
            "⚠️  Delete Package\n\n\
            Are you sure you want to delete '{package_name}'?\n\n\
            Type 'DELETE' below to confirm:"
        );

        let k = |a| config.keymap.get_key_display_for_action(a);
        let footer_text = format!("{}: Cancel", k(crate::keymap::Action::Quit));

        let dialog_height = 35;
        let dialog = Dialog::new("Delete Package", &content)
            .height(dialog_height)
            .variant(DialogVariant::Warning)
            .footer(&footer_text);
        frame.render_widget(dialog, area);

        // Render confirmation input below the dialog
        // Calculate dialog position to match Dialog's internal calculation
        let calculated_dialog_height =
            (f32::from(area.height) * (f32::from(dialog_height) / 100.0)) as u16;
        let dialog_y = area.y + (area.height.saturating_sub(calculated_dialog_height)) / 2;
        let input_y = dialog_y + calculated_dialog_height + 2; // 2 lines spacing

        if input_y + 3 <= area.height {
            // Center a 60-char wide input, matching dialog width approximately
            let input_width = 60.min(area.width);
            let input_x = area.x + (area.width.saturating_sub(input_width)) / 2;
            let input_area = Rect::new(input_x, input_y, input_width, 3);

            let widget = TextInputWidget::new(&self.state.delete_confirm_input)
                .title("Confirmation")
                .placeholder("Type 'DELETE' to confirm")
                .focused(true);
            frame.render_text_input_widget(widget, input_area);
        }

        Ok(())
    }

    fn render_installation_progress(&mut self, frame: &mut Frame, area: Rect) -> Result<()> {
        // Dialogs are rendered on top of main content (which is already rendered)
        // Dialog and Popup components handle their own background dimming

        match &self.state.installation_step {
            InstallationStep::NotStarted => {
                // Should not happen, but handle it
            }
            InstallationStep::Installing {
                package_index: _package_index,
                package_name,
                total_packages,
                packages_to_install,
                installed,
                failed,
                ..
            } => {
                use crate::components::Popup;
                let result = Popup::new()
                    .width(70)
                    .height(40)
                    .dim_background(true)
                    .render(frame, area);
                let popup_area = result.content_area;

                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Length(1), // Title
                        Constraint::Length(3), // Progress info
                        Constraint::Min(10),   // Output area
                        Constraint::Length(2), // Footer
                    ])
                    .split(popup_area);

                let t = theme();
                // Title
                let title = Paragraph::new("Installing Packages")
                    .alignment(Alignment::Center)
                    .style(t.title_style());
                frame.render_widget(title, chunks[0]);

                // Progress info
                // current_num is already 1-based (total - remaining = current position)
                let current_num = total_packages - packages_to_install.len();
                let progress_text = format!(
                    "Installing: {} ({}/{})\n\nPackages installed: {} | Failed: {}",
                    package_name,
                    current_num,
                    total_packages,
                    installed.len(),
                    failed.len()
                );
                let progress_para = Paragraph::new(progress_text)
                    .alignment(Alignment::Center)
                    .style(Style::default().fg(t.warning));
                frame.render_widget(progress_para, chunks[1]);

                // Output area (scrollable, auto-scrolls to bottom)
                let output_text: String = if self.state.installation_output.is_empty() {
                    "Installing...".to_string()
                } else {
                    self.state.installation_output.join("\n")
                };

                // Calculate scroll to show latest output
                // chunks[2] height minus borders (2 lines for top/bottom border)
                let visible_height = chunks[2].height.saturating_sub(2) as usize;
                let total_lines = self.state.installation_output.len();
                let scroll_offset = if total_lines > visible_height {
                    (total_lines - visible_height) as u16
                } else {
                    0
                };

                // Don't wrap output - package manager output is meant to be read as-is,
                // and wrapping breaks the scroll calculation (it counts logical lines, not display lines)
                let output_para = Paragraph::new(output_text)
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .border_type(theme().border_type(false))
                            .title(" Output "),
                    )
                    .scroll((scroll_offset, 0))
                    .style(t.text_style());
                frame.render_widget(output_para, chunks[2]);

                // Footer
                let footer_text = "Installing packages... (this may take a while)";
                Footer::render(frame, chunks[3], footer_text)?;
            }
            InstallationStep::Complete { installed, failed } => {
                use crate::widgets::{Dialog, DialogVariant};

                // Build summary content
                let mut summary = format!(
                    "✅ Successfully installed: {} package(s)\n",
                    installed.len()
                );
                if !failed.is_empty() {
                    summary.push_str(&format!("❌ Failed: {} package(s)\n\n", failed.len()));
                    summary.push_str("Failed packages:\n");
                    for (idx, error) in failed {
                        if let Some(pkg) = self.state.packages.get(*idx) {
                            summary.push_str(&format!("  • {}: {}\n", pkg.name, error));
                        }
                    }
                }

                let footer_text = "Press any key to continue";
                let dialog = Dialog::new("Installation Complete", &summary)
                    .height(30)
                    .variant(if failed.is_empty() {
                        DialogVariant::Default
                    } else {
                        DialogVariant::Warning
                    })
                    .footer(footer_text);
                frame.render_widget(dialog, area);
            }
        }

        Ok(())
    }

    fn render_install_missing_popup(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        config: &Config,
    ) -> Result<()> {
        use crate::widgets::{Dialog, DialogVariant};

        // Count missing packages
        let missing_count = self
            .state
            .package_statuses
            .iter()
            .filter(|s| matches!(s, PackageStatus::NotInstalled))
            .count();

        let missing_packages: Vec<String> = self
            .state
            .packages
            .iter()
            .enumerate()
            .filter_map(|(idx, pkg)| {
                if matches!(
                    self.state.package_statuses.get(idx),
                    Some(PackageStatus::NotInstalled)
                ) {
                    Some(pkg.name.clone())
                } else {
                    None
                }
            })
            .collect();

        // Build content message
        let message = if missing_count == 1 {
            "1 package is missing. Do you want to install it?".to_string()
        } else {
            format!("{missing_count} packages are missing. Do you want to install them?")
        };

        // Format package list
        let package_list_text = if missing_packages.is_empty() {
            String::new()
        } else {
            format!(
                "\n\nPackages to install:\n{}",
                missing_packages
                    .iter()
                    .map(|name| format!("  • {name}"))
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        };

        let content = format!("{message}{package_list_text}");

        let k = |a| config.keymap.get_key_display_for_action(a);
        let footer_text = format!(
            "{}: Install | {}: Cancel",
            k(crate::keymap::Action::Confirm),
            k(crate::keymap::Action::Cancel)
        );

        let dialog = Dialog::new("Install Missing Packages", &content)
            .height(25)
            .variant(DialogVariant::Warning)
            .footer(&footer_text);
        frame.render_widget(dialog, area);

        Ok(())
    }

    fn render_import_popup(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        config: &Config,
    ) -> Result<()> {
        use crate::components::Popup;
        use crate::ui::ImportFocus;
        use ratatui::symbols;

        let t = theme();

        // Create popup
        let result = Popup::new()
            .width(70)
            .height(80)
            .dim_background(true)
            .render(frame, area);
        let popup_area = result.content_area;

        // Determine if we should show tabs
        let show_tabs = self.state.import_available_sources.len() > 1;

        // Layout: Title, Tabs (optional), Content (bordered), Footer
        let outer_chunks = if show_tabs {
            Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(2), // Title
                    Constraint::Length(1), // Tabs
                    Constraint::Min(10),   // Content (bordered block with filter + list)
                    Constraint::Length(2), // Footer
                ])
                .split(popup_area)
        } else {
            Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(2), // Title
                    Constraint::Length(0), // No tabs
                    Constraint::Min(10),   // Content (bordered block with filter + list)
                    Constraint::Length(2), // Footer
                ])
                .split(popup_area)
        };

        // Title (generic)
        let title = Paragraph::new("Import System Packages")
            .alignment(Alignment::Center)
            .style(t.title_style());
        frame.render_widget(title, outer_chunks[0]);

        // Store tab area for mouse click-to-switch
        self.import_tabs_area = if show_tabs {
            Some(outer_chunks[1])
        } else {
            None
        };

        // Tabs (only if we have multiple sources)
        if show_tabs {
            self.render_import_tabs(frame, outer_chunks[1]);
        }

        // Content block with border (like ratatui tabs example)
        let content_border_style = self.import_content_border_style();
        let content_block = Block::bordered()
            .border_set(symbols::border::PROPORTIONAL_TALL)
            .border_style(content_border_style)
            .padding(Padding::horizontal(1));

        let content_inner = content_block.inner(outer_chunks[2]);
        frame.render_widget(content_block, outer_chunks[2]);

        // Split content area into filter and list
        let content_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Filter input
                Constraint::Min(5),    // List
            ])
            .split(content_inner);

        // Store filter and list areas for mouse support
        self.import_filter_area = Some(content_chunks[0]);
        self.import_list_area = Some(content_chunks[1]);

        // Filter input
        let filter_focused = self.state.import_focus == ImportFocus::Filter;
        let widget = TextInputWidget::new(&self.state.import_filter)
            .title("Filter")
            .placeholder("Type to filter packages...")
            .focused(filter_focused);
        frame.render_text_input_widget(widget, content_chunks[0]);

        // List (or loading state)
        self.render_import_list(frame, content_chunks[1]);

        // Footer
        self.render_import_footer(frame, outer_chunks[3], config)?;

        Ok(())
    }

    fn render_import_tabs(&self, frame: &mut Frame, area: Rect) {
        use crate::ui::ImportFocus;

        let t = theme();

        if self.state.import_available_sources.is_empty() {
            return;
        }

        let tabs_focused = self.state.import_focus == ImportFocus::Tabs;

        // Build tab titles with padding and background color (like ratatui example)
        let titles: Vec<Line> = self
            .state
            .import_available_sources
            .iter()
            .map(|source| {
                Line::from(format!("  {}  ", source.display_name()))
                    .fg(t.text_muted)
                    .bg(t.dim_bg)
            })
            .collect();

        // Highlight style for selected tab - brighter background when focused
        let highlight_style = if tabs_focused {
            Style::default().fg(t.background).bg(t.primary)
        } else {
            Style::default().fg(t.text).bg(t.text_muted)
        };

        // Build the tabs widget (like ratatui example)
        let tabs = Tabs::new(titles)
            .select(self.state.import_active_tab)
            .highlight_style(highlight_style)
            .padding("", "")
            .divider(" ");

        frame.render_widget(tabs, area);
    }

    /// Get the border style for import content areas (matches selected tab)
    fn import_content_border_style(&self) -> Style {
        use crate::ui::ImportFocus;

        let t = theme();
        let tabs_focused = self.state.import_focus == ImportFocus::Tabs;

        if tabs_focused {
            Style::default().fg(t.primary)
        } else {
            Style::default().fg(t.border)
        }
    }

    fn render_import_list(&mut self, frame: &mut Frame, area: Rect) {
        use crate::ui::ImportFocus;

        let t = theme();
        let list_focused = self.state.import_focus == ImportFocus::List;

        if self.state.import_loading {
            // Show loading spinner
            let spinner_frames = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
            let spinner = spinner_frames[self.state.import_spinner_tick % spinner_frames.len()];

            let center_y = area.y + area.height / 2 - 1;
            let centered_area = Rect::new(area.x, center_y, area.width, 3);

            let source_name = self
                .state
                .import_active_source()
                .map_or("packages", |s| s.display_name());

            let loading_text = format!(
                "{spinner} Discovering {source_name} packages...\n\nThis may take a moment"
            );
            let loading = Paragraph::new(loading_text)
                .alignment(Alignment::Center)
                .style(Style::default().fg(t.text_muted));
            frame.render_widget(loading, centered_area);
            return;
        }

        let packages = self.state.import_current_packages();

        if self.state.import_available_sources.is_empty() {
            let empty = Paragraph::new("No supported package managers found on this system.")
                .alignment(Alignment::Center)
                .style(Style::default().fg(t.text_muted));
            frame.render_widget(empty, area);
            return;
        }

        if packages.is_empty() {
            // Check if this source supports discovery
            let message = if let Some(source) = self.state.import_active_source() {
                if source.supports_discovery() {
                    "No packages found to import.\nAll installed packages may already be added."
                } else {
                    "Package discovery not yet supported for this manager.\nUse the Add button to add packages manually."
                }
            } else {
                "No packages found to import."
            };

            let empty = Paragraph::new(message)
                .alignment(Alignment::Center)
                .style(Style::default().fg(t.text_muted));
            frame.render_widget(empty, area);
            return;
        }

        let filtered = self.get_filtered_import_packages();

        if filtered.is_empty() {
            let empty = Paragraph::new("No packages match the filter.")
                .alignment(Alignment::Center)
                .style(Style::default().fg(t.text_muted));
            frame.render_widget(empty, area);
            return;
        }

        // Build list items
        let items: Vec<ListItem> = filtered
            .iter()
            .map(|&idx| {
                let pkg = &packages[idx];
                let is_selected = self.state.import_selected.contains(&idx);
                let checkbox = if is_selected { "[x]" } else { "[ ]" };
                let binary_info = pkg
                    .binary_name
                    .as_ref()
                    .filter(|b| *b != &pkg.package_name)
                    .map(|b| format!(" ({b})"))
                    .unwrap_or_default();

                let text = format!("{} {}{}", checkbox, pkg.package_name, binary_info);
                let style = if is_selected {
                    Style::default().fg(t.success)
                } else {
                    Style::default().fg(t.text)
                };
                ListItem::new(text).style(style)
            })
            .collect();

        let border_style = if list_focused {
            focused_border_style()
        } else {
            unfocused_border_style()
        };

        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(border_style)
                    .title(format!(
                        " {} packages ({} selected) ",
                        filtered.len(),
                        self.state.import_selected.len()
                    )),
            )
            .highlight_style(Style::default().bg(t.highlight_bg).fg(t.text))
            .highlight_symbol(crate::styles::LIST_HIGHLIGHT_SYMBOL);

        frame.render_stateful_widget(list, area, &mut self.state.import_list_state);

        // Populate mouse regions for clickable list items
        self.import_list_regions.clear();
        let inner = Block::default().borders(Borders::ALL).inner(area);
        let scroll_offset = self.state.import_list_state.offset();
        for (visible_i, &_filtered_idx) in filtered.iter().enumerate() {
            if visible_i < scroll_offset {
                continue;
            }
            let visible_row = (visible_i - scroll_offset) as u16;
            if visible_row >= inner.height {
                break;
            }
            let row_area = Rect::new(inner.x, inner.y + visible_row, inner.width, 1);
            self.import_list_regions.add(row_area, visible_i);
        }
    }

    fn render_import_footer(&self, frame: &mut Frame, area: Rect, config: &Config) -> Result<()> {
        use crate::ui::ImportFocus;

        let k = |a| config.keymap.get_key_display_for_action(a);
        let has_multiple_sources = self.state.import_available_sources.len() > 1;
        let tabs_focused = self.state.import_focus == ImportFocus::Tabs;

        let footer_text = if has_multiple_sources {
            if tabs_focused {
                // When tabs are focused, show arrow navigation hint
                format!(
                    "← →: Switch Tab | Tab: Focus | {}: Import | {}: Cancel",
                    k(crate::keymap::Action::Confirm),
                    k(crate::keymap::Action::Cancel),
                )
            } else {
                format!(
                    "{}: Toggle | Tab: Focus | {}: All | {}: None | {}: Import | {}: Cancel",
                    k(crate::keymap::Action::ToggleSelect),
                    k(crate::keymap::Action::SelectAll),
                    k(crate::keymap::Action::DeselectAll),
                    k(crate::keymap::Action::Confirm),
                    k(crate::keymap::Action::Cancel),
                )
            }
        } else {
            format!(
                "{}: Toggle | {}: All | {}: None | {}: Import | {}: Cancel",
                k(crate::keymap::Action::ToggleSelect),
                k(crate::keymap::Action::SelectAll),
                k(crate::keymap::Action::DeselectAll),
                k(crate::keymap::Action::Confirm),
                k(crate::keymap::Action::Cancel),
            )
        };

        Footer::render(frame, area, &footer_text)?;
        Ok(())
    }
}
