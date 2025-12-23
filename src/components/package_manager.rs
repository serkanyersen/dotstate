use crate::components::footer;
use crate::components::header::Header;
use crate::components::input_field::InputField;
use crate::config::Config;
use crate::ui::{
    AddPackageField, InstallationStep, PackageManagerState, PackagePopupType, PackageStatus,
};
use crate::utils::package_manager::PackageManagerImpl;
use crate::utils::profile_manifest::{Package, PackageManager};
use crate::utils::{center_popup, create_standard_layout};
use anyhow::Result;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap};
// Text input functions are used in app.rs, not here

/// Package manager component
pub struct PackageManagerComponent;

impl PackageManagerComponent {
    pub fn new() -> Self {
        Self
    }

    /// Render the package manager screen
    pub fn render_with_state(
        &self,
        frame: &mut Frame,
        area: Rect,
        state: &mut PackageManagerState,
        config: &Config,
        packages: &[Package],
    ) -> Result<()> {
        // Update packages in state - always sync from active profile
        state.packages = packages.to_vec();
        if state.package_statuses.len() != state.packages.len() {
            state.package_statuses = vec![PackageStatus::Unknown; state.packages.len()];
        }

        // Ensure list state is initialized if we have packages
        if !state.packages.is_empty() && state.list_state.selected().is_none() {
            state.list_state.select(Some(0));
        }

        // Check if popup is active or installation is in progress - if so, render dark background and popup/progress
        if state.popup_type != PackagePopupType::None {
            // Render dark background to dim the screen
            let background = Block::default().style(Style::default().bg(Color::Black));
            frame.render_widget(background, area);

            // Render popup
            self.render_popup(frame, area, state, config)?;
        } else if !matches!(state.installation_step, InstallationStep::NotStarted) {
            // Installation in progress - show progress screen
            self.render_installation_progress(frame, area, state)?;
        } else {
            // Normal rendering when no popup
            let layout = create_standard_layout(area, 5, 2);

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
            self.render_package_list(frame, chunks[0], state)?;

            // Right panel: Package details
            self.render_package_details(frame, chunks[1], state)?;

            // Footer
            let footer_text = if state.is_checking {
                "Checking packages..."
            } else if !matches!(state.installation_step, InstallationStep::NotStarted) {
                "Installing packages..."
            } else {
                "↑↓: Navigate | A: Add | E: Edit | D: Delete | C: Check All | S: Check Selected | I: Install Missing | Esc: Back"
            };
            footer::Footer::render(frame, layout.2, footer_text)?;
        }

        Ok(())
    }

    fn render_package_list(
        &self,
        frame: &mut Frame,
        area: Rect,
        state: &mut PackageManagerState,
    ) -> Result<()> {
        use crate::utils::{focused_border_style, unfocused_border_style};

        if state.packages.is_empty() {
            // Show empty state message
            let paragraph =
                Paragraph::new("No packages yet.\n\nPress 'A' to add your first package.")
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title("Packages")
                            .border_style(unfocused_border_style())
                            .padding(ratatui::widgets::Padding::new(1, 1, 1, 1)),
                    )
                    .wrap(Wrap { trim: true })
                    .alignment(Alignment::Center);
            frame.render_widget(paragraph, area);
        } else {
            let items: Vec<ListItem> = state
                .packages
                .iter()
                .enumerate()
                .map(|(idx, package)| {
                    let status_icon = match state.package_statuses.get(idx) {
                        Some(PackageStatus::Installed) => "✅",
                        Some(PackageStatus::NotInstalled) => "❌",
                        Some(PackageStatus::Error(_)) => "⚠️",
                        _ => {
                            if state.is_checking && state.checking_index == Some(idx) {
                                "🔄"
                            } else {
                                "  "
                            }
                        }
                    };

                    let text = format!("{} {}", status_icon, package.name);
                    let style = match state.package_statuses.get(idx) {
                        Some(PackageStatus::Installed) => Style::default().fg(Color::Green),
                        Some(PackageStatus::NotInstalled) => Style::default().fg(Color::Red),
                        Some(PackageStatus::Error(_)) => Style::default().fg(Color::Yellow),
                        _ => Style::default(),
                    };
                    ListItem::new(text).style(style)
                })
                .collect();

            let list = List::new(items)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("Packages")
                        .border_style(focused_border_style()),
                )
                .highlight_style(
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                );

            frame.render_stateful_widget(list, area, &mut state.list_state);
        }

        Ok(())
    }

    fn render_package_details(
        &self,
        frame: &mut Frame,
        area: Rect,
        state: &mut PackageManagerState,
    ) -> Result<()> {
        let selected = state.list_state.selected();
        let details = if let Some(idx) = selected {
            if let Some(package) = state.packages.get(idx) {
                self.format_package_details(package, state, idx)
            } else {
                "No package selected".to_string()
            }
        } else {
            "No package selected".to_string()
        };

        let paragraph = Paragraph::new(details)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Package Details"),
            )
            .wrap(Wrap { trim: true });

        frame.render_widget(paragraph, area);

        Ok(())
    }

    fn format_package_details(
        &self,
        package: &Package,
        state: &PackageManagerState,
        idx: usize,
    ) -> String {
        let mut details = format!("Name: {}\n", package.name);

        if let Some(desc) = &package.description {
            details.push_str(&format!("Description: {}\n", desc));
        }

        details.push_str(&format!("Manager: {:?}\n", package.manager));

        if let Some(pkg_name) = &package.package_name {
            details.push_str(&format!("Package Name: {}\n", pkg_name));
        }

        details.push_str(&format!("Binary Name: {}\n", package.binary_name));

        // Status
        let status = state.package_statuses.get(idx);
        match status {
            Some(PackageStatus::Installed) => details.push_str("\n\nStatus: ✅ Installed"),
            Some(PackageStatus::NotInstalled) => {
                details.push_str("\n\nStatus: ❌ Not Installed");
                // Check if manager is installed for installation purposes
                if !PackageManagerImpl::is_manager_installed(&package.manager) {
                    details.push_str(&format!(
                        "\n⚠️ Package manager '{:?}' is not installed",
                        package.manager
                    ));
                    details.push_str(&format!(
                        "\n\nInstallation instructions:\n{}",
                        PackageManagerImpl::installation_instructions(&package.manager)
                    ));
                }
            }
            Some(PackageStatus::Error(msg)) => {
                details.push_str(&format!("\n\nStatus: ⚠️ Error: {}", msg))
            }
            _ => details.push_str("\n\nStatus: ⏳ Unknown (press 'C' to check)"),
        }

        details
    }

    fn render_popup(
        &self,
        frame: &mut Frame,
        area: Rect,
        state: &mut PackageManagerState,
        config: &Config,
    ) -> Result<()> {
        match state.popup_type {
            PackagePopupType::Add | PackagePopupType::Edit => {
                self.render_add_edit_popup(frame, area, state, config)?;
            }
            PackagePopupType::Delete => {
                self.render_delete_popup(frame, area, state)?;
            }
            PackagePopupType::InstallMissing => {
                self.render_install_missing_popup(frame, area, state)?;
            }
            PackagePopupType::None => return Ok(()),
        }
        Ok(())
    }

    fn render_add_edit_popup(
        &self,
        frame: &mut Frame,
        area: Rect,
        state: &mut PackageManagerState,
        _config: &Config,
    ) -> Result<()> {
        // Make popup larger to fit all fields, especially for custom packages
        let popup_width = 80;
        let popup_height = if state.add_is_custom { 60 } else { 50 };
        let popup_area = center_popup(area, popup_width, popup_height);
        frame.render_widget(Clear, popup_area);

        let title = if state.add_editing_index.is_some() {
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

        if !state.add_is_custom {
            // Managed packages: Package Name, Binary Name
            constraints.push(Constraint::Length(3)); // Package name
            constraints.push(Constraint::Length(3)); // Binary name
        } else {
            // Custom packages: Binary Name, Install Command, Existence Check
            constraints.push(Constraint::Length(3)); // Binary name
            constraints.push(Constraint::Length(3)); // Install command
            constraints.push(Constraint::Length(3)); // Existence check
        }

        constraints.push(Constraint::Min(0)); // Spacer
        constraints.push(Constraint::Length(2)); // Footer

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(constraints)
            .split(popup_area);

        // Title (no border, just text)
        let title_para = Paragraph::new(title).alignment(Alignment::Center).style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );
        frame.render_widget(title_para, chunks[0]);

        // Name field
        InputField::render(
            frame,
            chunks[1],
            &state.add_name_input,
            state.add_name_cursor,
            state.add_focused_field == AddPackageField::Name,
            "Name",
            Some("Package display name"),
            Alignment::Left,
            false,
        )?;

        // Description field
        InputField::render(
            frame,
            chunks[2],
            &state.add_description_input,
            state.add_description_cursor,
            state.add_focused_field == AddPackageField::Description,
            "Description (optional)",
            Some("Package description"),
            Alignment::Left,
            false,
        )?;

        // Manager selection
        self.render_manager_selection(frame, chunks[3], state)?;

        let mut current_chunk = 4; // Start after title, name, description, manager

        if !state.add_is_custom {
            // Managed packages: Package Name, Binary Name, Manager Check
            InputField::render(
                frame,
                chunks[current_chunk],
                &state.add_package_name_input,
                state.add_package_name_cursor,
                state.add_focused_field == AddPackageField::PackageName,
                "Package Name",
                Some("Package name in manager (e.g., 'eza')"),
                Alignment::Left,
                false,
            )?;
            current_chunk += 1;

            InputField::render(
                frame,
                chunks[current_chunk],
                &state.add_binary_name_input,
                state.add_binary_name_cursor,
                state.add_focused_field == AddPackageField::BinaryName,
                "Binary Name",
                Some("Binary name to check (e.g., 'eza')"),
                Alignment::Left,
                false,
            )?;
        } else {
            // Custom packages: Binary Name, Install Command, Existence Check
            InputField::render(
                frame,
                chunks[current_chunk],
                &state.add_binary_name_input,
                state.add_binary_name_cursor,
                state.add_focused_field == AddPackageField::BinaryName,
                "Binary Name",
                Some("Binary name to check (e.g., 'mytool')"),
                Alignment::Left,
                false,
            )?;
            current_chunk += 1;

            InputField::render(
                frame,
                chunks[current_chunk],
                &state.add_install_command_input,
                state.add_install_command_cursor,
                state.add_focused_field == AddPackageField::InstallCommand,
                "Install Command",
                Some("Install command (e.g., './install.sh')"),
                Alignment::Left,
                false,
            )?;
            current_chunk += 1;

            InputField::render(
                frame,
                chunks[current_chunk],
                &state.add_existence_check_input,
                state.add_existence_check_cursor,
                state.add_focused_field == AddPackageField::ExistenceCheck,
                "Existence Check",
                Some("Command to check if package exists"),
                Alignment::Left,
                false,
            )?;
            current_chunk += 1;

            InputField::render(
                frame,
                chunks[current_chunk],
                &state.add_manager_check_input,
                state.add_manager_check_cursor,
                state.add_focused_field == AddPackageField::ManagerCheck,
                "Manager Check (optional)",
                Some("Custom manager check command (optional fallback)"),
                Alignment::Left,
                false,
            )?;
        }

        // Footer with instructions (always the last chunk)
        let footer_text = "Tab: Next field | Shift+Tab: Previous | Enter: Save | Esc: Cancel";
        footer::Footer::render(frame, chunks[chunks.len() - 1], footer_text)?;

        Ok(())
    }

    fn render_manager_selection(
        &self,
        frame: &mut Frame,
        area: Rect,
        state: &mut PackageManagerState,
    ) -> Result<()> {
        // Initialize available managers if empty
        if state.available_managers.is_empty() {
            state.available_managers = PackageManagerImpl::get_available_managers();
            if !state.available_managers.is_empty() {
                state.add_manager = Some(state.available_managers[0].clone());
                state.add_manager_selected = 0;
            }
        }

        // Create manager labels with selection state
        let manager_labels: Vec<(String, bool)> = state
            .available_managers
            .iter()
            .enumerate()
            .map(|(idx, manager)| {
                let is_selected = state.add_manager_selected == idx;
                let label = format!("{:?}", manager);
                (label, is_selected)
            })
            .collect();

        // Render checkboxes in a horizontal wrapping layout
        let block = Block::default()
            .borders(Borders::ALL)
            .title("Package Manager")
            .border_style(if state.add_focused_field == AddPackageField::Manager {
                crate::utils::focused_border_style()
            } else {
                crate::utils::unfocused_border_style()
            });

        let inner_area = block.inner(area);
        frame.render_widget(block, area);

        // Calculate how many checkboxes fit per row and render them
        let available_width = inner_area.width as usize;
        let mut current_x = 0;
        let mut current_y = 0;
        let line_height = 1;

        for (idx, (label, is_selected)) in manager_labels.iter().enumerate() {
            // Checkbox format: "[x] Label " or "[ ] Label "
            let checkbox_marker = if *is_selected { "[x]" } else { "[ ]" };
            let full_text = format!("{} {} ", checkbox_marker, label);
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
            let is_focused = state.add_focused_field == AddPackageField::Manager
                && state.add_manager_selected == idx;
            let checkbox_style = if is_focused {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else if *is_selected {
                Style::default().fg(Color::Green)
            } else {
                Style::default().fg(Color::White)
            };

            let checkbox_text = Paragraph::new(full_text).style(checkbox_style);
            frame.render_widget(checkbox_text, checkbox_area);

            // Update selected manager if this checkbox is selected
            if *is_selected {
                state.add_manager = Some(state.available_managers[idx].clone());
                state.add_manager_selected = idx;
                // Auto-detect if custom
                state.add_is_custom =
                    matches!(state.available_managers[idx], PackageManager::Custom);
            }

            current_x += checkbox_width;
        }

        Ok(())
    }

    fn render_delete_popup(
        &self,
        frame: &mut Frame,
        area: Rect,
        state: &mut PackageManagerState,
    ) -> Result<()> {
        let popup_area = center_popup(area, 50, 15);
        frame.render_widget(Clear, popup_area);

        let package_name = if let Some(idx) = state.delete_index {
            state
                .packages
                .get(idx)
                .map(|p| p.name.as_str())
                .unwrap_or("Unknown")
        } else {
            "Unknown"
        };

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(5), // Warning text
                Constraint::Length(3), // Confirmation input
                Constraint::Min(0),    // Spacer
                Constraint::Length(2), // Footer
            ])
            .split(popup_area);

        // Warning text
        let warning_text = format!(
            "⚠️  Delete Package\n\n\
            Are you sure you want to delete '{}'?\n\n\
            Type 'DELETE' to confirm:",
            package_name
        );

        let paragraph = Paragraph::new(warning_text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Delete Package"),
            )
            .wrap(Wrap { trim: true })
            .alignment(Alignment::Center);

        frame.render_widget(paragraph, chunks[0]);

        // Confirmation input
        InputField::render(
            frame,
            chunks[1],
            &state.delete_confirm_input,
            state.delete_confirm_cursor,
            true,
            "Confirmation",
            Some("Type 'DELETE' to confirm"),
            Alignment::Left,
            false,
        )?;

        // Footer
        footer::Footer::render(frame, chunks[3], "Enter: Confirm | Esc: Cancel")?;

        Ok(())
    }
}

impl PackageManagerComponent {
    fn render_installation_progress(
        &self,
        frame: &mut Frame,
        area: Rect,
        state: &mut PackageManagerState,
    ) -> Result<()> {
        use ratatui::style::{Color, Modifier, Style};

        // Render dark background
        let background = Block::default().style(Style::default().bg(Color::Black));
        frame.render_widget(background, area);

        match &state.installation_step {
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
                let popup_area = center_popup(area, 70, 40);
                frame.render_widget(Clear, popup_area);

                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Length(1), // Title
                        Constraint::Length(3), // Progress info
                        Constraint::Min(10),   // Output area
                        Constraint::Length(2), // Footer
                    ])
                    .split(popup_area);

                // Title
                let title = Paragraph::new("Installing Packages")
                    .alignment(Alignment::Center)
                    .style(
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    );
                frame.render_widget(title, chunks[0]);

                // Progress info
                let current_num = total_packages - packages_to_install.len();
                let progress_text = format!(
                    "Installing: {} ({}/{})\n\nPackages installed: {} | Failed: {}",
                    package_name,
                    current_num + 1,
                    total_packages,
                    installed.len(),
                    failed.len()
                );
                let progress_para = Paragraph::new(progress_text)
                    .alignment(Alignment::Center)
                    .style(Style::default().fg(Color::Yellow));
                frame.render_widget(progress_para, chunks[1]);

                // Output area (scrollable)
                let output_text: String = if state.installation_output.is_empty() {
                    "Installing...".to_string()
                } else {
                    state.installation_output.join("\n")
                };

                let output_para = Paragraph::new(output_text)
                    .block(Block::default().borders(Borders::ALL).title("Output"))
                    .wrap(Wrap { trim: true })
                    .style(Style::default().fg(Color::White));
                frame.render_widget(output_para, chunks[2]);

                // Footer
                let footer_text = "Installing packages... (this may take a while)";
                footer::Footer::render(frame, chunks[3], footer_text)?;
            }
            InstallationStep::Complete { installed, failed } => {
                // Show completion summary
                let popup_area = center_popup(area, 60, 30);
                frame.render_widget(Clear, popup_area);

                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Length(1), // Title
                        Constraint::Min(15),   // Summary
                        Constraint::Length(2), // Footer
                    ])
                    .split(popup_area);

                // Title
                let title = Paragraph::new("Installation Complete")
                    .alignment(Alignment::Center)
                    .style(
                        Style::default()
                            .fg(Color::Green)
                            .add_modifier(Modifier::BOLD),
                    );
                frame.render_widget(title, chunks[0]);

                // Summary
                let mut summary = format!(
                    "✅ Successfully installed: {} package(s)\n",
                    installed.len()
                );
                if !failed.is_empty() {
                    summary.push_str(&format!("❌ Failed: {} package(s)\n\n", failed.len()));
                    summary.push_str("Failed packages:\n");
                    for (idx, error) in failed {
                        if let Some(pkg) = state.packages.get(*idx) {
                            summary.push_str(&format!("  • {}: {}\n", pkg.name, error));
                        }
                    }
                }

                let summary_para = Paragraph::new(summary)
                    .block(Block::default().borders(Borders::ALL).title("Summary"))
                    .wrap(Wrap { trim: true })
                    .style(Style::default().fg(Color::White));
                frame.render_widget(summary_para, chunks[1]);

                // Footer
                let footer_text = "Press any key to continue";
                footer::Footer::render(frame, chunks[2], footer_text)?;
            }
        }

        Ok(())
    }

    fn render_install_missing_popup(
        &self,
        frame: &mut Frame,
        area: Rect,
        state: &mut PackageManagerState,
    ) -> Result<()> {
        let popup_area = center_popup(area, 60, 25);
        frame.render_widget(Clear, popup_area);

        // Count missing packages
        let missing_count = state
            .package_statuses
            .iter()
            .filter(|s| matches!(s, PackageStatus::NotInstalled))
            .count();

        let missing_packages: Vec<String> = state
            .packages
            .iter()
            .enumerate()
            .filter_map(|(idx, pkg)| {
                if matches!(
                    state.package_statuses.get(idx),
                    Some(PackageStatus::NotInstalled)
                ) {
                    Some(pkg.name.clone())
                } else {
                    None
                }
            })
            .collect();

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Title
                Constraint::Length(1), // Spacer
                Constraint::Length(3), // Message
                Constraint::Min(0),    // Package list
                Constraint::Length(1), // Spacer
                Constraint::Length(1), // Instructions
            ])
            .split(popup_area);

        // Title
        let title = Paragraph::new("Install Missing Packages")
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Package Manager")
                    .title_alignment(Alignment::Center)
                    .style(Style::default().bg(Color::Black)),
            )
            .alignment(Alignment::Center)
            .style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            );
        frame.render_widget(title, chunks[0]);

        // Message
        let message = if missing_count == 1 {
            "1 package is missing. Do you want to install it?".to_string()
        } else {
            format!(
                "{} packages are missing. Do you want to install them?",
                missing_count
            )
        };
        let message_para = Paragraph::new(message)
            .wrap(Wrap { trim: true })
            .style(Style::default().fg(Color::White));
        frame.render_widget(message_para, chunks[2]);

        // Package list
        if !missing_packages.is_empty() {
            let package_list: Vec<ListItem> = missing_packages
                .iter()
                .map(|name| {
                    ListItem::new(format!("  • {}", name)).style(Style::default().fg(Color::Cyan))
                })
                .collect();
            let list = List::new(package_list).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Packages to Install")
                    .border_style(Style::default().fg(Color::DarkGray)),
            );
            frame.render_widget(list, chunks[3]);
        }

        // Instructions
        let instructions = Paragraph::new("Press Y/Enter to install, N/Esc to cancel")
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(instructions, chunks[5]);

        Ok(())
    }
}
