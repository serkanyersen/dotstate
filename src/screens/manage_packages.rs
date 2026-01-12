use crate::components::package_manager::PackageManagerComponent;
use crate::config::Config;
use crate::keymap::{Action, Keymap};
use crate::screens::{RenderContext, Screen, ScreenAction, ScreenContext};
use crate::ui::{
    AddPackageField, InstallationStep, PackageManagerState, PackagePopupType, PackageStatus, Screen as ScreenEnum, InstallationStatus,
};
use crate::utils::package_installer::PackageInstaller;
use crate::utils::package_manager::PackageManagerImpl;
use crate::utils::profile_manifest::Package;
use crate::utils::text_input::{
    handle_backspace, handle_char_insertion, handle_cursor_movement, handle_delete,
};
use crate::services::PackageService;
use anyhow::Result;
use crossterm::event::{Event, KeyCode, KeyModifiers};
use ratatui::prelude::*;
use std::time::Duration;
use tracing::{debug, error, info, warn};

pub struct ManagePackagesScreen {
    component: PackageManagerComponent,
    pub state: PackageManagerState,
}

impl ManagePackagesScreen {
    pub fn new() -> Self {
        Self {
            component: PackageManagerComponent::new(),
            state: PackageManagerState::default(),
        }
    }

    pub fn get_state_mut(&mut self) -> &mut PackageManagerState {
        &mut self.state
    }

    pub fn update_packages(&mut self, packages: Vec<Package>) {
        // Only update if different to avoid resetting checking status unnecessarily
        // Actually, we should force update if we are entering the screen?
        // Let's just update and resize statuses.
        self.state.packages = packages;
        if self.state.package_statuses.len() != self.state.packages.len() {
            self.state.package_statuses = vec![PackageStatus::Unknown; self.state.packages.len()];
        }
    }

    pub fn reset_state(&mut self) {
         self.state.installation_step = InstallationStep::NotStarted;
         self.state.installation_output.clear();
         self.state.popup_type = PackagePopupType::None;
    }

    pub fn start_checking(&mut self) {
        let state = &mut self.state;
        state.is_checking = true;
        state.checking_index = None;
        state.checking_delay_until = Some(std::time::Instant::now() + Duration::from_millis(100));
        // Reset statuses to Unknown if we are re-checking
        state.package_statuses = vec![PackageStatus::Unknown; state.packages.len()];
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
             state.installation_delay_until = Some(std::time::Instant::now() + Duration::from_millis(100));
        }
    }

    fn get_action(&self, key: KeyCode, modifiers: KeyModifiers, keymap: &Keymap) -> Option<Action> {
        keymap.get_action(key, modifiers)
    }

    /// Process periodic tasks (package checking, installation monitoring)
    /// Returns ScreenAction::Refresh if a redraw is needed
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

        // 2. Process Installation (Stub for now, will implement logic similar to app.rs)
        if !matches!(self.state.installation_step, InstallationStep::NotStarted) {
             // In app.rs, process_installation_step handled the async receiver
             // We need to move that logic here or trigger it.
             // For now, let's just claim redraw if installing implementation details are moved here.
             // Wait, app.rs had `process_installation_step` which polled the receiver.
             // We need to port 'process_installation_step' logic here.
             self.process_installation_step()?;
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

        // If we have a specific index to check (from "Check Selected"), check only that one
        if let Some(index) = state.checking_index {
            if index < state.packages.len() {
                let package = &state.packages[index];
                info!("Checking selected package: {} (index: {})", package.name, index);

                match PackageInstaller::check_exists(package) {
                    Ok((true, _)) => {
                        state.package_statuses[index] = PackageStatus::Installed;
                    }
                    Ok((false, _)) => {
                         if !PackageManagerImpl::is_manager_installed(&package.manager) {
                            state.package_statuses[index] = PackageStatus::Error(format!(
                                "Package not found and package manager '{:?}' is not installed",
                                package.manager
                            ));
                        } else {
                            state.package_statuses[index] = PackageStatus::NotInstalled;
                        }
                    }
                    Err(e) => {
                        error!("Error checking package {}: {}", package.name, e);
                        state.package_statuses[index] = PackageStatus::Error(e.to_string());
                    }
                }

                state.checking_index = None;
                state.is_checking = false;
                state.checking_delay_until = None;
                return Ok(());
            } else {
                 state.checking_index = None;
                 state.is_checking = false;
                 return Ok(());
            }
        }

        // Check all packages sequentially (one per tick to keep UI responsive)
        // Find first 'Unknown' package
        if let Some(index) = state.package_statuses.iter().position(|s| matches!(s, PackageStatus::Unknown)) {
            let package = &state.packages[index];
            debug!("Checking package {}/{}", index + 1, state.packages.len());

             match PackageInstaller::check_exists(package) {
                    Ok((true, _)) => {
                        state.package_statuses[index] = PackageStatus::Installed;
                    }
                    Ok((false, _)) => {
                        if !PackageManagerImpl::is_manager_installed(&package.manager) {
                             state.package_statuses[index] = PackageStatus::Error(format!(
                                "Package not found and package manager '{:?}' is not installed",
                                package.manager
                            ));
                        } else {
                            state.package_statuses[index] = PackageStatus::NotInstalled;
                        }
                    }
                    Err(e) => {
                         state.package_statuses[index] = PackageStatus::Error(e.to_string());
                    }
            }

            // Schedule next check
            state.checking_delay_until = Some(std::time::Instant::now() + Duration::from_millis(10));
        } else {
            // All done
            state.is_checking = false;
            info!("Finished checking all packages");
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
                                         state.installation_output.push(format!("✅ Installed {}", package_name));
                                         // Update status in list
                                         if *package_index < state.package_statuses.len() {
                                             state.package_statuses[*package_index] = PackageStatus::Installed;
                                         }
                                     } else {
                                         let err_msg = error.unwrap_or_else(|| "Unknown error".to_string());
                                         error!("Failed to install {}: {}", package_name, err_msg);
                                         failed.push((*package_index, err_msg.clone()));
                                         state.installation_output.push(format!("❌ Failed to install {}: {}", package_name, err_msg));
                                     }
                                 }
                             }
                         }
                         Err(std::sync::mpsc::TryRecvError::Empty) => {
                             break;
                         }
                         Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                             // Thread died?
                              finished_current = true;
                              failed.push((*package_index, "Installation thread disconnected".to_string()));
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
         } = &state.installation_step {
             (packages_to_install.clone(), installed.clone(), failed.clone(), *total_packages)
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
              state.installation_delay_until = Some(std::time::Instant::now() + Duration::from_millis(500));
         }

         Ok(())
    }
}

impl Screen for ManagePackagesScreen {
    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: &RenderContext) -> Result<()> {
        let config = ctx.config;
        let packages = self.state.packages.clone();
        self.component.render_with_state(
            frame,
            area,
            &mut self.state,
            config,
            &packages,
        )?;
        Ok(())
    }

    fn handle_event(&mut self, event: Event, ctx: &ScreenContext) -> Result<ScreenAction> {
        let config = ctx.config;

        // If installation is in progress (and not complete), blocking input except maybe quit?
        // App.rs checks `installation_step` to show progress.
        if matches!(self.state.installation_step, InstallationStep::Installing { .. }) {
             // Block generic input, maybe allow generic Quit via App?
             // But actually, we just ignore most keys.
             return Ok(ScreenAction::None);
        }

        // If installation complete, any key closes it
        if let InstallationStep::Complete { .. } = self.state.installation_step {
             if let Event::Key(_) = event {
                 self.state.installation_step = InstallationStep::NotStarted;
                 self.state.installation_output.clear();
                 return Ok(ScreenAction::Refresh);
             }
             return Ok(ScreenAction::None);
        }

        if let Event::Key(key) = event {
            // Handle Popups
            match self.state.popup_type {
                PackagePopupType::Add | PackagePopupType::Edit => {
                    return self.handle_add_edit_popup_event(key, config);
                }
                PackagePopupType::Delete => {
                    return self.handle_delete_popup_event(key, config);
                }
                PackagePopupType::InstallMissing => {
                     // Just a list/info popup usually?
                     // In app.rs "Install Missing" wasn't a popup type with input, it was an Action that triggered logic.
                     // But `PackagePopupType::InstallMissing` exists in enum. Let's see if it's used.
                     // It is rendered in component.
                     // If it's a confirmation popup for install missing:
                     if let Some(action) = self.get_action(key.code, key.modifiers, &config.keymap) {
                          match action {
                               Action::Confirm => {
                                   // Start installation
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
                PackagePopupType::None => {
                    // Normal list navigation
                     if let Some(action) = self.get_action(key.code, key.modifiers, &config.keymap) {
                        return self.handle_main_list_action(action);
                     }
                }
            }
        }
        Ok(ScreenAction::None)
    }

    fn is_input_focused(&self) -> bool {
        matches!(
            self.state.popup_type,
            PackagePopupType::Add | PackagePopupType::Edit | PackagePopupType::Delete
        )
    }
}

impl ManagePackagesScreen {

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
                 if state.popup_type == PackagePopupType::None && !state.is_checking && !state.packages.is_empty() {
                     // Initialize check all
                     if state.package_statuses.len() != state.packages.len() {
                         state.package_statuses = vec![PackageStatus::Unknown; state.packages.len()];
                     }
                     state.package_statuses = vec![PackageStatus::Unknown; state.packages.len()];
                     state.is_checking = true;
                     state.checking_index = None;
                     state.checking_delay_until = Some(std::time::Instant::now() + Duration::from_millis(100));
                     return Ok(ScreenAction::Refresh);
                 }
             }
             Action::Sync => {
                 // Check Selected
                  if state.popup_type == PackagePopupType::None && !state.is_checking {
                      if let Some(idx) = state.list_state.selected() {
                          if idx < state.packages.len() {
                              // Reset status for this one
                                if state.package_statuses.len() != state.packages.len() {
                                     state.package_statuses = vec![PackageStatus::Unknown; state.packages.len()];
                                }
                                state.package_statuses[idx] = PackageStatus::Unknown;
                                state.is_checking = true;
                                state.checking_index = Some(idx);
                                state.checking_delay_until = Some(std::time::Instant::now() + Duration::from_millis(100));
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

                      let missing_count = state.package_statuses.iter().filter(|s| matches!(s, PackageStatus::NotInstalled)).count();
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
                             state.delete_confirm_cursor = 0;
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
             _ => {}
         }
         Ok(ScreenAction::None)
    }

    fn start_add_package(&mut self) -> Result<()> {
        let state = &mut self.state;
        state.popup_type = PackagePopupType::Add;
        state.add_editing_index = None;
        state.add_name_input.clear();
        state.add_name_cursor = 0;
        state.add_description_input.clear();
        state.add_description_cursor = 0;
        state.add_package_name_input.clear();
        state.add_package_name_cursor = 0;
        state.add_binary_name_input.clear();
        state.add_binary_name_cursor = 0;
        state.add_install_command_input.clear();
        state.add_install_command_cursor = 0;
        state.add_existence_check_input.clear();
        state.add_existence_check_cursor = 0;
        state.add_manager_check_input.clear();
        state.add_manager_check_cursor = 0;
        state.add_focused_field = AddPackageField::Name;
        // Initialize managers
        state.available_managers = PackageManagerImpl::get_available_managers();
        if !state.available_managers.is_empty() {
            state.add_manager = Some(state.available_managers[0].clone());
            state.add_manager_selected = 0;
            state.add_is_custom = matches!(state.available_managers[0], crate::utils::profile_manifest::PackageManager::Custom);
        }
        state.manager_list_state.select(Some(0));
        Ok(())
    }

    fn start_edit_package(&mut self, index: usize) -> Result<()> {
          let state = &mut self.state;
          if let Some(pkg) = state.packages.get(index) {
                state.popup_type = PackagePopupType::Edit;
                state.add_editing_index = Some(index);
                state.add_name_input = pkg.name.clone();
                state.add_name_cursor = pkg.name.chars().count();
                state.add_description_input = pkg.description.clone().unwrap_or_default();
                state.add_description_cursor = state.add_description_input.chars().count();
                state.add_package_name_input = pkg.package_name.clone().unwrap_or_default();
                state.add_package_name_cursor = state.add_package_name_input.chars().count();
                state.add_binary_name_input = pkg.binary_name.clone();
                state.add_binary_name_cursor = pkg.binary_name.chars().count();
                 state.add_install_command_input = pkg.install_command.clone().unwrap_or_default();
                state.add_install_command_cursor = state.add_install_command_input.chars().count();
                state.add_existence_check_input = pkg.existence_check.clone().unwrap_or_default();
                state.add_existence_check_cursor = state.add_existence_check_input.chars().count();
                state.add_manager_check_input = pkg.manager_check.clone().unwrap_or_default();
                state.add_manager_check_cursor = state.add_manager_check_input.chars().count();

                 state.available_managers = PackageManagerImpl::get_available_managers();
                 state.add_manager = Some(pkg.manager.clone());
                 state.add_is_custom = matches!(pkg.manager, crate::utils::profile_manifest::PackageManager::Custom);
                 if let Some(pos) = state.available_managers.iter().position(|m| *m == pkg.manager) {
                     state.add_manager_selected = pos;
                 } else {
                     state.add_manager_selected = 0;
                 }
                 state.manager_list_state.select(Some(state.add_manager_selected));

                state.add_focused_field = AddPackageField::Name;
          }
          Ok(())
    }

    fn handle_add_edit_popup_event(&mut self, key: crossterm::event::KeyEvent, config: &Config) -> Result<ScreenAction> {
        let action = config.keymap.get_action(key.code, key.modifiers);
        let state = &mut self.state;

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
                             state.add_manager = Some(state.available_managers[state.add_manager_selected].clone());
                             state.add_is_custom = matches!(state.available_managers[state.add_manager_selected], crate::utils::profile_manifest::PackageManager::Custom);
                         }
                    } else {
                        // Save
                         let (name, description, package_name, binary_name, install_command, existence_check, manager_check, manager, is_custom, edit_idx) = (
                            state.add_name_input.clone(),
                            state.add_description_input.clone(),
                            state.add_package_name_input.clone(),
                            state.add_binary_name_input.clone(),
                            state.add_install_command_input.clone(),
                            state.add_existence_check_input.clone(),
                            state.add_manager_check_input.clone(),
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
                            // Show error (maybe via log or state message? - Log for now)
                             warn!("Package validation failed: {:?}", validation.error_message);
                             // Ideally we show a message popup, but we don't have direct access here?
                             // We can return ScreenAction::ShowMessage!
                             if let Some(msg) = validation.error_message {
                                 return Ok(ScreenAction::ShowMessage {
                                     title: "Validation Error".to_string(),
                                     content: msg,
                                 });
                             }
                             return Ok(ScreenAction::None);
                        }

                        let manager = manager.unwrap(); // Validated
                        let package = PackageService::create_package(
                             &name,
                             &description,
                             manager,
                             is_custom,
                             &package_name,
                             &binary_name,
                             &install_command,
                             &existence_check,
                             &manager_check,
                        );

                        // Save
                        let repo_path = &config.repo_path;
                        let active_profile = &config.active_profile;
                         let packages = if let Some(idx) = edit_idx {
                            PackageService::update_package(repo_path, active_profile, idx, package)?
                        } else {
                            PackageService::add_package(repo_path, active_profile, package)?
                        };

                        self.update_packages(packages);
                        self.reset_state();
                        return Ok(ScreenAction::Refresh);
                    }
                    return Ok(ScreenAction::Refresh);
                }
                Action::MoveUp | Action::MoveDown => {
                     if state.add_focused_field == AddPackageField::Manager {
                          let count = state.available_managers.len();
                          if count > 0 {
                               if matches!(action, Action::MoveDown) {
                                   state.add_manager_selected = (state.add_manager_selected + 1) % count;
                               } else {
                                   state.add_manager_selected = if state.add_manager_selected == 0 { count - 1 } else { state.add_manager_selected - 1 };
                               }
                               state.add_manager = Some(state.available_managers[state.add_manager_selected].clone());
                               state.add_is_custom = matches!(state.available_managers[state.add_manager_selected], crate::utils::profile_manifest::PackageManager::Custom);
                               state.manager_list_state.select(Some(state.add_manager_selected));
                          }
                          return Ok(ScreenAction::Refresh);
                     }
                }
                Action::MoveLeft | Action::MoveRight | Action::Home | Action::End | Action::Backspace | Action::DeleteChar => {
                     // Handled below with text input helpers
                }
                 _ => {}
            }
        }

        // Text input handling
        let key_code = match action {
            Some(Action::MoveLeft) => Some(KeyCode::Left),
            Some(Action::MoveRight) => Some(KeyCode::Right),
            Some(Action::Home) => Some(KeyCode::Home),
            Some(Action::End) => Some(KeyCode::End),
            _ => None,
        };

        if let Some(k) = key_code {
             match state.add_focused_field {
                AddPackageField::Name => handle_cursor_movement(&state.add_name_input, &mut state.add_name_cursor, k),
                AddPackageField::Description => handle_cursor_movement(&state.add_description_input, &mut state.add_description_cursor, k),
                AddPackageField::PackageName => handle_cursor_movement(&state.add_package_name_input, &mut state.add_package_name_cursor, k),
                AddPackageField::BinaryName => handle_cursor_movement(&state.add_binary_name_input, &mut state.add_binary_name_cursor, k),
                AddPackageField::InstallCommand => handle_cursor_movement(&state.add_install_command_input, &mut state.add_install_command_cursor, k),
                AddPackageField::ExistenceCheck => handle_cursor_movement(&state.add_existence_check_input, &mut state.add_existence_check_cursor, k),
                _ => {}
            }
            return Ok(ScreenAction::Refresh);
        }

        if let Some(Action::Backspace) = action {
             match state.add_focused_field {
                AddPackageField::Name => handle_backspace(&mut state.add_name_input, &mut state.add_name_cursor),
                AddPackageField::Description => handle_backspace(&mut state.add_description_input, &mut state.add_description_cursor),
                AddPackageField::PackageName => {
                    handle_backspace(&mut state.add_package_name_input, &mut state.add_package_name_cursor);
                    // Update binary name suggestion
                     let new_suggestion = PackageManagerImpl::suggest_binary_name(&state.add_package_name_input);
                     if state.add_binary_name_input.is_empty() { // Simplification: Only if empty for now, or elaborate logic if needed
                         state.add_binary_name_input = new_suggestion;
                         state.add_binary_name_cursor = state.add_binary_name_input.chars().count();
                     }
                },
                AddPackageField::BinaryName => handle_backspace(&mut state.add_binary_name_input, &mut state.add_binary_name_cursor),
                AddPackageField::InstallCommand => handle_backspace(&mut state.add_install_command_input, &mut state.add_install_command_cursor),
                AddPackageField::ExistenceCheck => handle_backspace(&mut state.add_existence_check_input, &mut state.add_existence_check_cursor),
                _ => {}
            }
            return Ok(ScreenAction::Refresh);
        }

        if let Some(Action::DeleteChar) = action {
             match state.add_focused_field {
                AddPackageField::Name => handle_delete(&mut state.add_name_input, &mut state.add_name_cursor),
                AddPackageField::Description => handle_delete(&mut state.add_description_input, &mut state.add_description_cursor),
                AddPackageField::PackageName => handle_delete(&mut state.add_package_name_input, &mut state.add_package_name_cursor),
                AddPackageField::BinaryName => handle_delete(&mut state.add_binary_name_input, &mut state.add_binary_name_cursor),
                AddPackageField::InstallCommand => handle_delete(&mut state.add_install_command_input, &mut state.add_install_command_cursor),
                AddPackageField::ExistenceCheck => handle_delete(&mut state.add_existence_check_input, &mut state.add_existence_check_cursor),
                _ => {}
            }
            return Ok(ScreenAction::Refresh);
        }

        // Char input
        if let KeyCode::Char(c) = key.code {
            if !key.modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER) {
                 match state.add_focused_field {
                    AddPackageField::Name => handle_char_insertion(&mut state.add_name_input, &mut state.add_name_cursor, c),
                    AddPackageField::Description => handle_char_insertion(&mut state.add_description_input, &mut state.add_description_cursor, c),
                    AddPackageField::PackageName => {
                        handle_char_insertion(&mut state.add_package_name_input, &mut state.add_package_name_cursor, c);
                         // Update binary name suggestion
                         let new_suggestion = PackageManagerImpl::suggest_binary_name(&state.add_package_name_input);
                         if state.add_binary_name_input.is_empty() {
                             state.add_binary_name_input = new_suggestion;
                             state.add_binary_name_cursor = state.add_binary_name_input.chars().count();
                         }
                    }
                    AddPackageField::BinaryName => handle_char_insertion(&mut state.add_binary_name_input, &mut state.add_binary_name_cursor, c),
                    AddPackageField::InstallCommand => handle_char_insertion(&mut state.add_install_command_input, &mut state.add_install_command_cursor, c),
                    AddPackageField::ExistenceCheck => handle_char_insertion(&mut state.add_existence_check_input, &mut state.add_existence_check_cursor, c),
                    _ => {}
                }
                return Ok(ScreenAction::Refresh);
            }
        }

        Ok(ScreenAction::None)
    }

    fn handle_delete_popup_event(&mut self, key: crossterm::event::KeyEvent, config: &Config) -> Result<ScreenAction> {
          let action = config.keymap.get_action(key.code, key.modifiers);
          let state = &mut self.state;

          if let Some(action) = action {
              match action {
                  Action::Cancel => {
                      self.reset_state();
                      return Ok(ScreenAction::Refresh);
                  }
                  Action::Confirm => {
                       if state.delete_confirm_input.trim() == "DELETE" {
                           if let Some(idx) = state.delete_index {
                               let packages = PackageService::delete_package(&config.repo_path, &config.active_profile, idx)?;
                               self.update_packages(packages);
                               self.reset_state();
                               return Ok(ScreenAction::Refresh);
                           }
                       }
                  }
                  Action::Backspace => {
                      handle_backspace(&mut state.delete_confirm_input, &mut state.delete_confirm_cursor);
                      return Ok(ScreenAction::Refresh);
                  }
                  Action::DeleteChar => {
                      handle_delete(&mut state.delete_confirm_input, &mut state.delete_confirm_cursor);
                      return Ok(ScreenAction::Refresh);
                  }
                  Action::MoveLeft => {
                      handle_cursor_movement(&state.delete_confirm_input, &mut state.delete_confirm_cursor, KeyCode::Left);
                      return Ok(ScreenAction::Refresh);
                  }
                   Action::MoveRight => {
                      handle_cursor_movement(&state.delete_confirm_input, &mut state.delete_confirm_cursor, KeyCode::Right);
                      return Ok(ScreenAction::Refresh);
                  }
                   Action::Home => {
                      handle_cursor_movement(&state.delete_confirm_input, &mut state.delete_confirm_cursor, KeyCode::Home);
                      return Ok(ScreenAction::Refresh);
                  }
                   Action::End => {
                      handle_cursor_movement(&state.delete_confirm_input, &mut state.delete_confirm_cursor, KeyCode::End);
                      return Ok(ScreenAction::Refresh);
                  }
                  _ => {}
              }
          }

          if let KeyCode::Char(c) = key.code {
               if !key.modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER) {
                   handle_char_insertion(&mut state.delete_confirm_input, &mut state.delete_confirm_cursor, c);
                   return Ok(ScreenAction::Refresh);
               }
          }

          Ok(ScreenAction::None)
    }


}
