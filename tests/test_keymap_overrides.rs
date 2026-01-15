use crossterm::event::{KeyCode, KeyModifiers};
use dotstate::config::Config;
use dotstate::keymap::{Action, KeyBinding, KeymapPreset};
use tempfile::TempDir;

#[test]
fn test_keymap_override_in_config() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("config.toml");
    let repo_path = temp_dir.path().join("repo");

    // Create a config with keymap overrides
    let mut config = Config {
        repo_path: repo_path.clone(),
        ..Default::default()
    };
    config.keymap.preset = KeymapPreset::Vim;

    // Add an override: map 'x' to Quit (normally 'q' in vim preset)
    config
        .keymap
        .overrides
        .push(KeyBinding::new("x", Action::Quit));
    // Add another override: map 'w' to MoveUp (normally 'k' in vim preset)
    config
        .keymap
        .overrides
        .push(KeyBinding::new("w", Action::MoveUp));

    // Save config
    config.save(&config_path).unwrap();

    // Load config back
    let loaded = Config::load_or_create(&config_path).unwrap();

    // Verify preset is preserved
    assert_eq!(loaded.keymap.preset, KeymapPreset::Vim);

    // Verify overrides are loaded
    assert_eq!(loaded.keymap.overrides.len(), 2);
    assert!(loaded
        .keymap
        .overrides
        .iter()
        .any(|b| b.key == "x" && b.action == Action::Quit));
    assert!(loaded
        .keymap
        .overrides
        .iter()
        .any(|b| b.key == "w" && b.action == Action::MoveUp));

    // Test that overrides take precedence
    // 'x' should map to Quit (override), not the default vim binding
    let action = loaded
        .keymap
        .get_action(KeyCode::Char('x'), KeyModifiers::NONE);
    assert_eq!(action, Some(Action::Quit));

    // 'w' should map to MoveUp (override)
    let action = loaded
        .keymap
        .get_action(KeyCode::Char('w'), KeyModifiers::NONE);
    assert_eq!(action, Some(Action::MoveUp));

    // 'q' should NOT map to Quit anymore - preset binding is shadowed by 'x' override
    // When an action is overridden, all preset bindings for that action are removed
    let action = loaded
        .keymap
        .get_action(KeyCode::Char('q'), KeyModifiers::NONE);
    assert_eq!(action, None);

    // 'k' should NOT map to MoveUp anymore - preset binding is shadowed by 'w' override
    let action = loaded
        .keymap
        .get_action(KeyCode::Char('k'), KeyModifiers::NONE);
    assert_eq!(action, None);

    // Other vim bindings should still work (not overridden)
    // 'j' should still map to MoveDown (not overridden)
    let action = loaded
        .keymap
        .get_action(KeyCode::Char('j'), KeyModifiers::NONE);
    assert_eq!(action, Some(Action::MoveDown));
}

#[test]
fn test_keymap_override_shadows_preset() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("config.toml");
    let repo_path = temp_dir.path().join("repo");

    // Create a config with keymap override that shadows a preset binding
    let mut config = Config {
        repo_path: repo_path.clone(),
        ..Default::default()
    };
    config.keymap.preset = KeymapPreset::Vim;

    // Override 'j' to map to MoveUp instead of MoveDown
    config
        .keymap
        .overrides
        .push(KeyBinding::new("j", Action::MoveUp));

    // Save and load
    config.save(&config_path).unwrap();
    let loaded = Config::load_or_create(&config_path).unwrap();

    // 'j' should now map to MoveUp (override), not MoveDown (preset)
    let action = loaded
        .keymap
        .get_action(KeyCode::Char('j'), KeyModifiers::NONE);
    assert_eq!(action, Some(Action::MoveUp));

    // 'k' should NOT map to MoveUp anymore - preset binding is shadowed by override
    // This is the correct behavior: when an action is overridden, preset bindings for that action are removed
    let action = loaded
        .keymap
        .get_action(KeyCode::Char('k'), KeyModifiers::NONE);
    assert_eq!(action, None);

    // 'h' should still work for MoveLeft (not overridden)
    let action = loaded
        .keymap
        .get_action(KeyCode::Char('h'), KeyModifiers::NONE);
    assert_eq!(action, Some(Action::MoveLeft));
}

#[test]
fn test_keymap_override_with_modifiers() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("config.toml");
    let repo_path = temp_dir.path().join("repo");

    // Create a config with keymap override using modifiers
    let mut config = Config {
        repo_path: repo_path.clone(),
        ..Default::default()
    };
    config.keymap.preset = KeymapPreset::Standard;

    // Override Ctrl+N to map to Quit
    config
        .keymap
        .overrides
        .push(KeyBinding::new("ctrl+n", Action::Quit));

    // Save and load
    config.save(&config_path).unwrap();
    let loaded = Config::load_or_create(&config_path).unwrap();

    // Ctrl+N should map to Quit (override)
    let action = loaded
        .keymap
        .get_action(KeyCode::Char('n'), KeyModifiers::CONTROL);
    assert_eq!(action, Some(Action::Quit));

    // Plain 'n' should not map to Quit
    let action = loaded
        .keymap
        .get_action(KeyCode::Char('n'), KeyModifiers::NONE);
    assert_ne!(action, Some(Action::Quit));
}

#[test]
fn test_keymap_override_serialization_format() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("config.toml");
    let repo_path = temp_dir.path().join("repo");

    // Create a config with keymap overrides
    let mut config = Config {
        repo_path: repo_path.clone(),
        ..Default::default()
    };
    config.keymap.preset = KeymapPreset::Emacs;
    config
        .keymap
        .overrides
        .push(KeyBinding::new("f1", Action::Help));
    config
        .keymap
        .overrides
        .push(KeyBinding::new("ctrl+h", Action::Quit));

    // Save config
    config.save(&config_path).unwrap();

    // Read the raw config file to verify format
    let content = std::fs::read_to_string(&config_path).unwrap();

    // Verify it contains keymap section
    assert!(content.contains("[keymap]"));
    // Preset is serialized as lowercase
    assert!(content.contains("preset = \"emacs\""));
    assert!(content.contains("overrides"));

    // Load it back to verify it works
    let loaded = Config::load_or_create(&config_path).unwrap();
    assert_eq!(loaded.keymap.preset, KeymapPreset::Emacs);
    assert_eq!(loaded.keymap.overrides.len(), 2);

    // Verify the overrides work
    let action = loaded.keymap.get_action(KeyCode::F(1), KeyModifiers::NONE);
    assert_eq!(action, Some(Action::Help));

    let action = loaded
        .keymap
        .get_action(KeyCode::Char('h'), KeyModifiers::CONTROL);
    assert_eq!(action, Some(Action::Quit));
}
