//! The [`DotstateLogo`] widget renders the dotstate logo.
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::Text;
use ratatui::widgets::Widget;

/// A widget that renders the dotstate logo
///
/// The dotstate logo comes in two sizes: `Tiny` (2 lines) and `Small` (3 lines).
/// This may be used in an application's welcome screen or about screen.
///
/// # Examples
///
/// ```rust
/// use dotstate::widgets::DotstateLogo;
///
/// # fn draw(frame: &mut ratatui::Frame) {
/// frame.render_widget(DotstateLogo::tiny(), frame.area());
/// # }
/// ```
///
/// Renders:
///
/// ```text
///  ⡏⢱ ⢀⡀ ⣰⡀ ⢎⡑ ⣰⡀ ⢀⣀ ⣰⡀ ⢀⡀
///  ⠧⠜ ⠣⠜ ⠘⠤ ⠢⠜ ⠘⠤ ⠣⠼ ⠘⠤ ⠣⠭
/// ```
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct DotstateLogo {
    size: Size,
}

/// The size of the logo
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Size {
    /// A tiny logo (2 lines, braille characters)
    ///
    /// ```text
    ///  ⡏⢱ ⢀⡀ ⣰⡀ ⢎⡑ ⣰⡀ ⢀⣀ ⣰⡀ ⢀⡀
    ///  ⠧⠜ ⠣⠜ ⠘⠤ ⠢⠜ ⠘⠤ ⠣⠼ ⠘⠤ ⠣⠭
    /// ```
    #[default]
    Tiny,
    /// A small logo (3 lines, box drawing characters)
    ///
    /// ```text
    /// ╺┳┓┏━┓╺┳╸┏━┓╺┳╸┏━┓╺┳╸┏━╸
    ///  ┃┃┃ ┃ ┃ ┗━┓ ┃ ┣━┫ ┃ ┣╸
    /// ╺┻┛┗━┛ ╹ ┗━┛ ╹ ╹ ╹ ╹ ┗━╸
    /// ```
    Small,
}

impl DotstateLogo {
    /// Create a new dotstate logo widget
    ///
    /// # Examples
    ///
    /// ```
    /// use dotstate::widgets::{DotstateLogo, Size};
    ///
    /// let logo = DotstateLogo::new(Size::Tiny);
    /// ```
    pub const fn new(size: Size) -> Self {
        Self { size }
    }

    /// Create a new dotstate logo widget with a tiny size (2 lines)
    ///
    /// # Examples
    ///
    /// ```
    /// use dotstate::widgets::DotstateLogo;
    ///
    /// let logo = DotstateLogo::tiny();
    /// ```
    #[allow(dead_code)] // Keeping this utility method for future use
    pub const fn tiny() -> Self {
        Self::new(Size::Tiny)
    }

    /// Create a new dotstate logo widget with a small size (3 lines)
    ///
    /// # Examples
    ///
    /// ```
    /// use dotstate::widgets::DotstateLogo;
    ///
    /// let logo = DotstateLogo::small();
    /// ```
    pub const fn small() -> Self {
        Self::new(Size::Small)
    }
}

impl Widget for DotstateLogo {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let logo = self.size.as_str();
        Text::raw(logo).render(area, buf);
    }
}

impl Size {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Tiny => Self::tiny(),
            Self::Small => Self::small(),
        }
    }

    const fn tiny() -> &'static str {
        " ⡏⢱ ⢀⡀ ⣰⡀ ⢎⡑ ⣰⡀ ⢀⣀ ⣰⡀ ⢀⡀\n ⠧⠜ ⠣⠜ ⠘⠤ ⠢⠜ ⠘⠤ ⠣⠼ ⠘⠤ ⠣⠭"
    }

    const fn small() -> &'static str {
        "╺┳┓┏━┓╺┳╸┏━┓╺┳╸┏━┓╺┳╸┏━╸\n ┃┃┃ ┃ ┃ ┗━┓ ┃ ┣━┫ ┃ ┣╸ \n╺┻┛┗━┛ ╹ ┗━┛ ╹ ╹ ╹ ╹ ┗━╸"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_size() {
        let logo = DotstateLogo::new(Size::Tiny);
        assert_eq!(logo.size, Size::Tiny);
    }

    #[test]
    fn default_logo_is_tiny() {
        let logo = DotstateLogo::default();
        assert_eq!(logo.size, Size::Tiny);
    }

    #[test]
    fn tiny_logo_constant() {
        let logo = DotstateLogo::tiny();
        assert_eq!(logo.size, Size::Tiny);
    }

    #[test]
    fn small_logo_constant() {
        let logo = DotstateLogo::small();
        assert_eq!(logo.size, Size::Small);
    }
}
