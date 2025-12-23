use anyhow::{bail, Result};

/// Reserved profile names that cannot be used
const RESERVED_NAMES: &[&str] = &["backup", "temp", ".git", "node_modules", "target", "build"];

/// Maximum profile name length
const MAX_NAME_LENGTH: usize = 50;

/// Validate a profile name
///
/// # Arguments
/// * `name` - The profile name to validate
/// * `existing_profiles` - List of existing profile names
///
/// # Returns
/// * `Ok(())` if valid
/// * `Err` with descriptive message if invalid
///
/// # Rules
/// - Must be 1-50 characters
/// - Only alphanumeric, hyphens, and underscores
/// - Cannot be a reserved name
/// - Must be unique (case-insensitive)
/// - Cannot start with a dot
/// - Cannot be only whitespace
pub fn validate_profile_name(name: &str, existing_profiles: &[String]) -> Result<()> {
    let trimmed = name.trim();

    // Check if empty or only whitespace
    if trimmed.is_empty() {
        bail!("Profile name cannot be empty");
    }

    // Check length
    if trimmed.len() > MAX_NAME_LENGTH {
        bail!(
            "Profile name must be {} characters or less (got {})",
            MAX_NAME_LENGTH,
            trimmed.len()
        );
    }

    // Check for valid characters (alphanumeric, hyphens, underscores)
    if !trimmed
        .chars()
        .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    {
        bail!("Profile name can only contain letters, numbers, hyphens, and underscores");
    }

    // Cannot start with dot (hidden files/folders)
    if trimmed.starts_with('.') {
        bail!("Profile name cannot start with a dot");
    }

    // Check reserved names (case-insensitive)
    let lower_name = trimmed.to_lowercase();
    if RESERVED_NAMES.contains(&lower_name.as_str()) {
        bail!("'{}' is a reserved name and cannot be used", trimmed);
    }

    // Check uniqueness (case-insensitive)
    if existing_profiles
        .iter()
        .any(|p| p.eq_ignore_ascii_case(trimmed))
    {
        bail!("A profile with the name '{}' already exists", trimmed);
    }

    Ok(())
}

/// Sanitize a profile name to make it safe for use as a folder name
///
/// # Arguments
/// * `name` - The profile name to sanitize
///
/// # Returns
/// * Sanitized name that's safe to use as a folder name
///
/// # Transformations
/// - Trim whitespace
/// - Replace spaces with hyphens
/// - Remove invalid characters
/// - Truncate to MAX_NAME_LENGTH
pub fn sanitize_profile_name(name: &str) -> String {
    name.trim()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else if c.is_whitespace() {
                '-'
            } else {
                '_'
            }
        })
        .collect::<String>()
        .chars()
        .take(MAX_NAME_LENGTH)
        .collect()
}

/// Check if a profile name is safe (no validation against existing, just format)
///
/// # Arguments
/// * `name` - The profile name to check
///
/// # Returns
/// * `true` if the name is safe to use as a folder name
#[allow(dead_code)] // Kept for potential future use in CLI or programmatic access
pub fn is_safe_profile_name(name: &str) -> bool {
    let trimmed = name.trim();

    !trimmed.is_empty()
        && trimmed.len() <= MAX_NAME_LENGTH
        && !trimmed.starts_with('.')
        && trimmed
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
        && !RESERVED_NAMES.contains(&trimmed.to_lowercase().as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_names() {
        let existing = vec![];

        assert!(validate_profile_name("Personal-Mac", &existing).is_ok());
        assert!(validate_profile_name("Work_Linux", &existing).is_ok());
        assert!(validate_profile_name("Home-Server-2024", &existing).is_ok());
        assert!(validate_profile_name("test123", &existing).is_ok());
    }

    #[test]
    fn test_invalid_names() {
        let existing = vec![];

        // Empty
        assert!(validate_profile_name("", &existing).is_err());
        assert!(validate_profile_name("   ", &existing).is_err());

        // Invalid characters
        assert!(validate_profile_name("Profile Name", &existing).is_err());
        assert!(validate_profile_name("Profile@Home", &existing).is_err());
        assert!(validate_profile_name("Test/Profile", &existing).is_err());

        // Reserved names
        assert!(validate_profile_name("backup", &existing).is_err());
        assert!(validate_profile_name("temp", &existing).is_err());
        assert!(validate_profile_name(".git", &existing).is_err());

        // Starts with dot
        assert!(validate_profile_name(".hidden", &existing).is_err());

        // Too long
        let long_name = "a".repeat(51);
        assert!(validate_profile_name(&long_name, &existing).is_err());
    }

    #[test]
    fn test_duplicate_names() {
        let existing = vec!["Personal".to_string(), "Work".to_string()];

        // Exact match
        assert!(validate_profile_name("Personal", &existing).is_err());

        // Case-insensitive match
        assert!(validate_profile_name("personal", &existing).is_err());
        assert!(validate_profile_name("PERSONAL", &existing).is_err());
        assert!(validate_profile_name("work", &existing).is_err());

        // Should be OK
        assert!(validate_profile_name("Home", &existing).is_ok());
    }

    #[test]
    fn test_sanitize_name() {
        assert_eq!(sanitize_profile_name("Personal Mac"), "Personal-Mac");
        assert_eq!(sanitize_profile_name("Work @ Office"), "Work-_-Office");
        assert_eq!(sanitize_profile_name("  test  "), "test");
        assert_eq!(sanitize_profile_name("Test/Profile"), "Test_Profile");

        // Long name should be truncated
        let long_name = "a".repeat(60);
        assert_eq!(sanitize_profile_name(&long_name).len(), MAX_NAME_LENGTH);
    }

    #[test]
    fn test_is_safe_name() {
        assert!(is_safe_profile_name("Personal-Mac"));
        assert!(is_safe_profile_name("Work_Linux"));
        assert!(is_safe_profile_name("test123"));

        assert!(!is_safe_profile_name(""));
        assert!(!is_safe_profile_name("Profile Name"));
        assert!(!is_safe_profile_name(".hidden"));
        assert!(!is_safe_profile_name("backup"));

        let long_name = "a".repeat(51);
        assert!(!is_safe_profile_name(&long_name));
    }
}
