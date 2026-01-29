//! The [`DotstateLogo`] widget renders the dotstate logo.
use crate::styles::theme;
use indoc::indoc;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::Text;
use ratatui::widgets::Widget;

/// A widget that renders the dotstate logo
///
/// The dotstate logo comes in two sizes: `Small` (2 lines) and `regular` (3 lines).
/// This may be used in an application's welcome screen or about screen.
///
/// # Examples
///
/// ```rust
/// use dotstate::widgets::DotstateLogo;
///
/// # fn draw(frame: &mut ratatui::Frame) {
/// frame.render_widget(DotstateLogo::small(), frame.area());
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
    /// A small logo (2 lines, braille characters)
    ///
    /// ```text
    ///  ⡏⢱ ⢀⡀ ⣰⡀ ⢎⡑ ⣰⡀ ⢀⣀ ⣰⡀ ⢀⡀
    ///  ⠧⠜ ⠣⠜ ⠘⠤ ⠢⠜ ⠘⠤ ⠣⠼ ⠘⠤ ⠣⠭
    /// ```
    #[default]
    Small,
    /// A regular logo (3 lines, box drawing characters)
    ///
    /// ```text
    /// ╺┳┓┏━┓╺┳╸┏━┓╺┳╸┏━┓╺┳╸┏━╸
    ///  ┃┃┃ ┃ ┃ ┗━┓ ┃ ┣━┫ ┃ ┣╸
    /// ╺┻┛┗━┛ ╹ ┗━┛ ╹ ╹ ╹ ╹ ┗━╸
    /// ```
    Regular,
    /// The classic logo (3 lines, box drawing characters)
    ///
    /// ```text
    /// ╔╦╗╔═╗╔╦╗╔═╗╔╦╗╔═╗╔╦╗╔═╗
    ///  ║║║ ║ ║ ╚═╗ ║ ╠═╣ ║ ║╣
    /// ═╩╝╚═╝ ╩ ╚═╝ ╩ ╩ ╩ ╩ ╚═╝
    /// ```
    Classic,
    /// A narrow logo (3 lines, box drawing characters)
    ///
    /// ```text
    /// ┳┓┏┓┏┳┓┏┓┏┳┓┏┓┏┳┓┏┓
    /// ┃┃┃┃ ┃ ┗┓ ┃ ┣┫ ┃ ┣
    /// ┻┛┗┛ ┻ ┗┛ ┻ ┛┗ ┻ ┗┛
    /// ```
    Narrow,
}

impl DotstateLogo {
    /// Create a new dotstate logo widget
    ///
    /// # Examples
    ///
    /// ```
    /// use dotstate::widgets::{DotstateLogo, Size};
    ///
    /// let logo = DotstateLogo::new(Size::Small);
    /// ```
    #[must_use]
    pub const fn new(size: Size) -> Self {
        Self { size }
    }

    /// Create a new dotstate logo widget with a small size (2 lines)
    ///
    /// # Examples
    ///
    /// ```
    /// use dotstate::widgets::DotstateLogo;
    ///
    /// let logo = DotstateLogo::small();
    /// ```
    #[allow(dead_code)] // Keeping this utility method for future use
    #[must_use]
    pub const fn small() -> Self {
        Self::new(Size::Small)
    }

    /// Create a new dotstate logo widget with a regular size (3 lines)
    ///
    /// # Examples
    ///
    /// ```
    /// use dotstate::widgets::DotstateLogo;
    ///
    /// let logo = DotstateLogo::regular();
    /// ```
    #[must_use]
    pub const fn regular() -> Self {
        Self::new(Size::Regular)
    }

    /// Create a new dotstate logo widget with a classic size (3 lines)
    ///
    /// # Examples
    ///
    /// ```rust
    /// use dotstate::widgets::DotstateLogo;
    ///
    /// let logo = DotstateLogo::classic();
    /// ```
    #[must_use]
    pub const fn classic() -> Self {
        Self::new(Size::Classic)
    }

    /// Create a new dotstate logo widget with a narrow size (3 lines)
    ///
    /// # Examples
    ///
    /// ```rust
    /// use dotstate::widgets::DotstateLogo;
    ///
    /// let logo = DotstateLogo::narrow();
    /// ```
    #[must_use]
    pub const fn narrow() -> Self {
        Self::new(Size::Narrow)
    }

    /// Returns the width of the logo in terminal cells
    #[must_use]
    pub const fn width(&self) -> u16 {
        self.size.width()
    }

    /// Returns the height of the logo in lines
    #[must_use]
    pub const fn height(&self) -> u16 {
        self.size.height()
    }
}

impl Widget for DotstateLogo {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let logo = self.size.as_str();
        Text::raw(logo)
            .style(theme().text_style())
            .render(area, buf);
    }
}

impl Size {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Small => Self::small(),
            Self::Regular => Self::regular(),
            Self::Classic => Self::classic(),
            Self::Narrow => Self::narrow(),
        }
    }

    /// Returns the width of the logo in terminal cells
    #[must_use]
    pub const fn width(self) -> u16 {
        match self {
            Self::Small => 16,   // "▄ ▄▖▄▖▄▖▄▖▄▖▄▖▄▖"
            Self::Regular => 24, // "╺┳┓┏━┓╺┳╸┏━┓╺┳╸┏━┓╺┳╸┏━╸"
            Self::Classic => 24, // "╔╦╗╔═╗╔╦╗╔═╗╔╦╗╔═╗╔╦╗╔═╗"
            Self::Narrow => 19,  // "┳┓┏┓┏┳┓┏┓┏┳┓┏┓┏┳┓┏┓"
        }
    }

    /// Returns the height of the logo in lines
    #[must_use]
    pub const fn height(self) -> u16 {
        match self {
            Self::Small => 3,
            Self::Regular => 3,
            Self::Classic => 3,
            Self::Narrow => 3,
        }
    }

    const fn small() -> &'static str {
        indoc! {"
            ▄ ▄▖▄▖▄▖▄▖▄▖▄▖▄▖
            ▌▌▌▌▐ ▚ ▐ ▌▌▐ ▙▖
            ▙▘▙▌▐ ▄▌▐ ▛▌▐ ▙▖
        "}
    }

    const fn regular() -> &'static str {
        indoc! {"
            ╺┳┓┏━┓╺┳╸┏━┓╺┳╸┏━┓╺┳╸┏━╸
             ┃┃┃ ┃ ┃ ┗━┓ ┃ ┣━┫ ┃ ┣╸
            ╺┻┛┗━┛ ╹ ┗━┛ ╹ ╹ ╹ ╹ ┗━╸
        "}
    }

    const fn classic() -> &'static str {
        indoc! {"
            ╔╦╗╔═╗╔╦╗╔═╗╔╦╗╔═╗╔╦╗╔═╗
             ║║║ ║ ║ ╚═╗ ║ ╠═╣ ║ ║╣
            ═╩╝╚═╝ ╩ ╚═╝ ╩ ╩ ╩ ╩ ╚═╝
        "}
    }

    const fn narrow() -> &'static str {
        indoc! {"
            ┳┓┏┓┏┳┓┏┓┏┳┓┏┓┏┳┓┏┓
            ┃┃┃┃ ┃ ┗┓ ┃ ┣┫ ┃ ┣
            ┻┛┗┛ ┻ ┗┛ ┻ ┛┗ ┻ ┗┛
        "}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_size() {
        let logo = DotstateLogo::new(Size::Small);
        assert_eq!(logo.size, Size::Small);
    }

    #[test]
    fn default_logo_is_small() {
        let logo = DotstateLogo::default();
        assert_eq!(logo.size, Size::Small);
    }

    #[test]
    fn small_logo_constant() {
        let logo = DotstateLogo::small();
        assert_eq!(logo.size, Size::Small);
    }

    #[test]
    fn regular_logo_constant() {
        let logo = DotstateLogo::regular();
        assert_eq!(logo.size, Size::Regular);
    }

    #[test]
    fn logo_dimensions_match_content() {
        for size in [Size::Small, Size::Regular, Size::Classic, Size::Narrow] {
            let content = size.as_str();
            let lines: Vec<&str> = content.lines().collect();

            // Check height
            assert_eq!(
                lines.len() as u16,
                size.height(),
                "{size:?} height mismatch"
            );

            // Check width (max line width in chars - all logo chars are 1 cell wide)
            let max_width = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0) as u16;
            assert_eq!(max_width, size.width(), "{size:?} width mismatch");
        }
    }
}
