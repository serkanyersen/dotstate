use crossterm::event::{KeyCode, KeyModifiers};
use dotstate::keymap::{Action, KeyBinding, Keymap, KeymapPreset};

#[test]
fn test_override_shadows_preset_binding() {
    // Test that when move_up is overridden, the original preset key (k) no longer works
    let keymap = Keymap {
        preset: KeymapPreset::Vim,
        overrides: vec![KeyBinding::new("w", Action::MoveUp)],
    };

    // 'w' should map to MoveUp (override)
    assert_eq!(
        keymap.get_action(KeyCode::Char('w'), KeyModifiers::NONE),
        Some(Action::MoveUp)
    );

    // 'k' should NOT map to MoveUp anymore (preset binding is shadowed)
    // Instead, it should return None since move_up is overridden
    assert_eq!(
        keymap.get_action(KeyCode::Char('k'), KeyModifiers::NONE),
        None
    );

    // 'j' should still work for MoveDown (not overridden)
    assert_eq!(
        keymap.get_action(KeyCode::Char('j'), KeyModifiers::NONE),
        Some(Action::MoveDown)
    );
}

#[test]
fn test_display_reflects_overrides() {
    let keymap = Keymap {
        preset: KeymapPreset::Vim,
        overrides: vec![
            KeyBinding::new("w", Action::MoveUp),
            KeyBinding::new("x", Action::Quit),
        ],
    };

    // Navigation display should show W/j (w overrides k, display is uppercase)
    let nav_display = keymap.navigation_display();
    assert!(
        nav_display.contains('W') || nav_display.contains('w'),
        "nav_display should contain 'w', got: '{nav_display}'"
    );
    assert!(
        nav_display.contains('j') || nav_display.contains('J'),
        "nav_display should contain 'j', got: '{nav_display}'"
    );
    assert!(
        !nav_display.contains('k') && !nav_display.contains('K'),
        "nav_display should not contain 'k' since it's shadowed, got: '{nav_display}'"
    );

    // Quit display should show X (override) instead of q
    let quit_display = keymap.quit_display();
    assert!(
        quit_display.contains('x') || quit_display.contains('X'),
        "quit_display should contain 'x', got: '{quit_display}'"
    );

    // Verify that get_key_display_for_action returns the override
    assert_eq!(keymap.get_key_display_for_action(Action::MoveUp), "W");
    assert_eq!(keymap.get_key_display_for_action(Action::Quit), "X");

    // Confirm should still show Enter (not overridden)
    let confirm_display = keymap.confirm_display();
    assert!(
        confirm_display.contains("Enter") || confirm_display.contains("enter"),
        "confirm_display should contain 'Enter', got: '{confirm_display}'"
    );
}
