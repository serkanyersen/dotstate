#[test]
fn test_example_config_loads() {
    use crossterm::event::{KeyCode, KeyModifiers};
    use dotstate::config::Config;
    use dotstate::keymap::{Action, KeymapPreset};
    use std::fs;
    use tempfile::TempDir;

    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("config.toml");

    // Copy the example config file content
    let example_content = r#"
active_profile = "default"
repo_path = "/tmp/dotstate-storage"
repo_name = "dotstate-storage"
default_branch = "main"
backup_enabled = true
profile_activated = false
custom_files = []
theme = "dark"

[keymap]
preset = "vim"

[[keymap.overrides]]
key = "x"
action = "quit"

[[keymap.overrides]]
key = "w"
action = "move_up"

[[keymap.overrides]]
key = "ctrl+h"
action = "help"
"#;

    fs::write(&config_path, example_content).unwrap();

    // Load the config
    let config = Config::load_or_create(&config_path).unwrap();

    // Verify preset
    assert_eq!(config.keymap.preset, KeymapPreset::Vim);

    // Verify overrides work
    let action = config
        .keymap
        .get_action(KeyCode::Char('x'), KeyModifiers::NONE);
    assert_eq!(action, Some(Action::Quit));

    let action = config
        .keymap
        .get_action(KeyCode::Char('w'), KeyModifiers::NONE);
    assert_eq!(action, Some(Action::MoveUp));

    let action = config
        .keymap
        .get_action(KeyCode::Char('h'), KeyModifiers::CONTROL);
    assert_eq!(action, Some(Action::Help));
}
