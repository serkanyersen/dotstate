/// Shared loading-spinner frames (braille pattern animation).
///
/// Used by screens with background operations so every spinner in the
/// app animates identically. Index with a wrapping tick counter via
/// [`spinner_frame`].
const SPINNER_FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// Return the spinner frame for the given tick counter.
#[must_use]
pub fn spinner_frame(tick: usize) -> &'static str {
    SPINNER_FRAMES[tick % SPINNER_FRAMES.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spinner_frames_are_non_empty() {
        assert_eq!(SPINNER_FRAMES.len(), 10);
        assert!(SPINNER_FRAMES.iter().all(|frame| !frame.is_empty()));
    }

    #[test]
    fn test_spinner_frame_wraps_around() {
        assert_eq!(spinner_frame(0), SPINNER_FRAMES[0]);
        assert_eq!(spinner_frame(SPINNER_FRAMES.len()), SPINNER_FRAMES[0]);
        assert_eq!(
            spinner_frame(2 * SPINNER_FRAMES.len() + 3),
            SPINNER_FRAMES[3]
        );
    }
}
