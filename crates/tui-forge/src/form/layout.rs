//! Form layout strategies and height-calculation helpers.
//!
//! The actual rendering is driven by [`Form::render`](super::Form::render) in
//! `mod.rs`, but this module provides the layout enum and the constraint
//! calculations that `render` delegates to.

/// Layout strategy for arranging form fields.
#[derive(Debug, Clone, Default)]
pub enum FormLayout {
    /// Stack fields vertically, one per row.
    ///
    /// Each field gets:
    /// - label row (1 line, if a label is set)
    /// - field widget (height from `FormField::height()`)
    /// - error row  (1 line, if an error is present)
    /// - spacing    (1 blank line between fields)
    #[default]
    Vertical,

    /// Arrange fields in a grid with the given number of columns.
    ///
    /// Fields wrap to the next row when the column count is reached.
    /// Each row's height is determined by the tallest field in that row
    /// (including its label and error lines).
    Grid {
        /// Number of columns in the grid.
        columns: u16,
    },
}

/// Calculate the total height for a single field entry (label + field + error).
///
/// # Arguments
/// * `label`        - The optional label text. If `Some`, adds 1 row.
/// * `field_height` - The widget height reported by `FormField::height()`.
/// * `has_error`    - Whether there is a validation error to display (adds 1 row).
///
/// # Returns
/// The total height in terminal rows for this single field entry,
/// **not** including inter-field spacing.
pub fn calculate_field_height(label: Option<&str>, field_height: u16, has_error: bool) -> u16 {
    let label_height: u16 = if label.is_some() { 1 } else { 0 };
    let error_height: u16 = if has_error { 1 } else { 0 };
    label_height + field_height + error_height
}

/// Calculate the total height required to render all form fields in the
/// given layout.
///
/// # Arguments
/// * `entries` - Slice of `(label, field_height, has_error)` tuples, one
///   per field, in registration order.
/// * `layout`  - The active [`FormLayout`].
///
/// # Returns
/// Total height in terminal rows, including inter-field / inter-row spacing.
pub fn calculate_total_height(
    entries: &[(Option<&str>, u16, bool)],
    layout: &FormLayout,
) -> u16 {
    if entries.is_empty() {
        return 0;
    }

    match layout {
        FormLayout::Vertical => {
            let mut total: u16 = 0;
            for (i, (label, fh, has_err)) in entries.iter().enumerate() {
                total += calculate_field_height(*label, *fh, *has_err);
                // Add 1 row spacing between fields (not after the last one)
                if i + 1 < entries.len() {
                    total += 1;
                }
            }
            total
        }
        FormLayout::Grid { columns } => {
            let cols = (*columns).max(1) as usize;
            let mut total: u16 = 0;
            let mut row_idx = 0;

            // Process entries in chunks of `cols`
            let chunks: Vec<&[(Option<&str>, u16, bool)]> = entries.chunks(cols).collect();
            let num_rows = chunks.len();

            for chunk in &chunks {
                // The row height is the maximum field height in this row
                let row_height = chunk
                    .iter()
                    .map(|(label, fh, has_err)| calculate_field_height(*label, *fh, *has_err))
                    .max()
                    .unwrap_or(0);

                total += row_height;

                // Add 1 row spacing between grid rows (not after the last one)
                row_idx += 1;
                if row_idx < num_rows {
                    total += 1;
                }
            }

            total
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_field_height_label_and_error() {
        // label(1) + field(1) + error(1) = 3
        assert_eq!(calculate_field_height(Some("Name"), 1, true), 3);
    }

    #[test]
    fn test_field_height_no_label_no_error() {
        // field(1) only
        assert_eq!(calculate_field_height(None, 1, false), 1);
    }

    #[test]
    fn test_field_height_label_no_error() {
        // label(1) + field(3) = 4  (e.g. a textarea)
        assert_eq!(calculate_field_height(Some("Bio"), 3, false), 4);
    }

    #[test]
    fn test_total_height_vertical_empty() {
        assert_eq!(calculate_total_height(&[], &FormLayout::Vertical), 0);
    }

    #[test]
    fn test_total_height_vertical_single() {
        let entries = vec![(Some("Name"), 1u16, false)];
        // label(1) + field(1) = 2, no spacing
        assert_eq!(calculate_total_height(&entries, &FormLayout::Vertical), 2);
    }

    #[test]
    fn test_total_height_vertical_multiple() {
        let entries = vec![
            (Some("Name"), 1, false),   // 1 + 1 = 2
            (None, 1, true),            // 0 + 1 + 1 = 2
            (Some("Bio"), 3, false),    // 1 + 3 = 4
        ];
        // 2 + 1(spacing) + 2 + 1(spacing) + 4 = 10
        assert_eq!(calculate_total_height(&entries, &FormLayout::Vertical), 10);
    }

    #[test]
    fn test_total_height_grid_two_columns() {
        let entries = vec![
            (Some("First"), 1, false),  // height = 2
            (Some("Last"), 1, true),    // height = 3  -> row max = 3
            (Some("Email"), 1, false),  // height = 2  -> row max = 2
        ];
        // row1: 3 + spacing(1) + row2: 2 = 6
        let layout = FormLayout::Grid { columns: 2 };
        assert_eq!(calculate_total_height(&entries, &layout), 6);
    }

    #[test]
    fn test_total_height_grid_single_row() {
        let entries = vec![
            (None, 1, false), // 1
            (None, 1, false), // 1
        ];
        // single row, max = 1, no spacing
        let layout = FormLayout::Grid { columns: 3 };
        assert_eq!(calculate_total_height(&entries, &layout), 1);
    }
}
