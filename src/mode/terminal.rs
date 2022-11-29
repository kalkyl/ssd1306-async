use crate::{command::AddrMode, mode::DisplayConfig, rotation::DisplayRotation, size::*, Ssd1306};
use crate::{DisplayError, WriteOnlyDataCommand};
use core::{cmp::min, fmt};

/// Extends the [`DisplaySize`](crate::size::DisplaySize) trait
/// to include number of characters that can fit on the display.
pub trait TerminalDisplaySize: DisplaySize {
    /// The number of characters that can fit on the display at once (w * h / (8 * 8))
    const CHAR_NUM: u8;
}

impl TerminalDisplaySize for DisplaySize128x64 {
    const CHAR_NUM: u8 = 128;
}

impl TerminalDisplaySize for DisplaySize128x32 {
    const CHAR_NUM: u8 = 64;
}

impl TerminalDisplaySize for DisplaySize96x16 {
    const CHAR_NUM: u8 = 24;
}

impl TerminalDisplaySize for DisplaySize72x40 {
    const CHAR_NUM: u8 = 45;
}

impl TerminalDisplaySize for DisplaySize64x48 {
    const CHAR_NUM: u8 = 48;
}

/// Contains the new row that the cursor has wrapped around to
struct CursorWrapEvent(u8);

#[derive(Copy, Clone, Debug)]
struct Cursor {
    col: u8,
    row: u8,
    width: u8,
    height: u8,
}

impl Cursor {
    pub fn new(width_pixels: u8, height_pixels: u8) -> Self {
        let width = width_pixels / 8;
        let height = height_pixels / 8;
        Cursor {
            col: 0,
            row: 0,
            width,
            height,
        }
    }

    /// Advances the logical cursor by one character.
    /// Returns a value indicating if this caused the cursor to wrap to the next line or the next
    /// screen.
    pub fn advance(&mut self) -> Option<CursorWrapEvent> {
        self.col = (self.col + 1) % self.width;
        if self.col == 0 {
            self.row = (self.row + 1) % self.height;
            Some(CursorWrapEvent(self.row))
        } else {
            None
        }
    }

    /// Advances the logical cursor to the start of the next line
    /// Returns a value indicating the now active line
    pub fn advance_line(&mut self) -> CursorWrapEvent {
        self.row = (self.row + 1) % self.height;
        self.col = 0;
        CursorWrapEvent(self.row)
    }

    /// Sets the position of the logical cursor arbitrarily.
    /// The position will be capped at the maximal possible position.
    pub fn set_position(&mut self, col: u8, row: u8) {
        self.col = min(col, self.width - 1);
        self.row = min(row, self.height - 1);
    }

    /// Gets the position of the logical cursor on screen in (col, row) order
    pub fn get_position(&self) -> (u8, u8) {
        (self.col, self.row)
    }

    /// Gets the logical dimensions of the screen in terms of characters, as (width, height)
    pub fn get_dimensions(&self) -> (u8, u8) {
        (self.width, self.height)
    }
}

/// Errors which can occur when interacting with the terminal mode
#[derive(Clone)]
pub enum TerminalModeError {
    /// An error occurred in the underlying interface layer
    InterfaceError(DisplayError),
    /// The mode was used before it was initialized
    Uninitialized,
    /// A location was specified outside the bounds of the screen
    OutOfBounds,
}

impl core::fmt::Debug for TerminalModeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> Result<(), core::fmt::Error> {
        match self {
            Self::InterfaceError(_) => "InterfaceError".fmt(f),
            Self::Uninitialized => "Uninitialized".fmt(f),
            Self::OutOfBounds => "OutOfBound".fmt(f),
        }
    }
}

// Cannot use From<_> due to coherence
trait IntoTerminalModeResult<T> {
    fn terminal_err(self) -> Result<T, TerminalModeError>;
}

impl<T> IntoTerminalModeResult<T> for Result<T, DisplayError> {
    fn terminal_err(self) -> Result<T, TerminalModeError> {
        self.map_err(TerminalModeError::InterfaceError)
    }
}

/// Terminal mode.
#[derive(Copy, Clone, Debug)]
pub struct TerminalMode {
    cursor: Option<Cursor>,
}

impl TerminalMode {
    /// Create a new terminal mode config instance.
    pub fn new() -> Self {
        Self { cursor: None }
    }
}

impl<DI, SIZE> DisplayConfig for Ssd1306<DI, SIZE, TerminalMode>
where
    DI: WriteOnlyDataCommand<Error = DisplayError>,
    SIZE: TerminalDisplaySize,
{
    type Error = TerminalModeError;

    /// Set the display rotation
    ///
    /// This method resets the cursor but does not clear the screen.
    async fn set_rotation(&mut self, rot: DisplayRotation) -> Result<(), Self::Error> {
        self.set_rotation(rot).await.terminal_err()?;
        // Need to reset cursor position, otherwise coordinates can become invalid
        self.reset_pos().await
    }

    /// Initialise the display in page mode (i.e. a byte walks down a column of 8 pixels) with
    /// column 0 on the left and column _(SIZE::Width::U8 - 1)_ on the right, but no automatic line
    /// wrapping.
    async fn init(&mut self) -> Result<(), Self::Error> {
        self.init_with_addr_mode(AddrMode::Page)
            .await
            .terminal_err()?;
        self.reset_pos().await
    }
}

impl<DI, SIZE> Ssd1306<DI, SIZE, TerminalMode>
where
    DI: WriteOnlyDataCommand<Error = DisplayError>,
    SIZE: TerminalDisplaySize,
{
    /// Clear the display and reset the cursor to the top left corner
    pub async fn clear(&mut self) -> Result<(), TerminalModeError> {
        // Let the chip handle line wrapping so we can fill the screen with blanks faster
        self.set_addr_mode(AddrMode::Horizontal)
            .await
            .terminal_err()?;

        let offset_x = match self.rotation() {
            DisplayRotation::Rotate0 | DisplayRotation::Rotate270 => SIZE::OFFSETX,
            DisplayRotation::Rotate180 | DisplayRotation::Rotate90 => {
                // If segment remapping is flipped, we need to calculate
                // the offset from the other edge of the display.
                SIZE::DRIVER_COLS - SIZE::WIDTH - SIZE::OFFSETX
            }
        };
        self.set_draw_area(
            (offset_x, SIZE::OFFSETY),
            (SIZE::WIDTH + offset_x, SIZE::HEIGHT + SIZE::OFFSETY),
        )
        .await
        .terminal_err()?;

        // Clear the display
        for _ in 0..SIZE::CHAR_NUM {
            self.draw(&[0; 8]).await.terminal_err()?;
        }

        // But for normal operation we manage the line wrapping
        self.set_addr_mode(AddrMode::Page).await.terminal_err()?;
        self.reset_pos().await?;

        Ok(())
    }

    /// Print a character to the display
    pub async fn print_char(&mut self, c: char) -> Result<(), TerminalModeError> {
        match c {
            '\n' => {
                let CursorWrapEvent(new_line) = self.ensure_cursor()?.advance_line();
                self.set_column(0).await.terminal_err()?;
                self.set_row(new_line * 8).await.terminal_err()?;
            }
            '\r' => {
                self.set_column(0).await.terminal_err()?;
                let (_, cur_line) = self.ensure_cursor()?.get_position();
                self.ensure_cursor()?.set_position(0, cur_line);
            }
            _ => {
                let bitmap = match self.rotation {
                    DisplayRotation::Rotate0 | DisplayRotation::Rotate180 => {
                        Self::char_to_bitmap(c)
                    }
                    DisplayRotation::Rotate90 | DisplayRotation::Rotate270 => {
                        let bitmap = Self::char_to_bitmap(c);
                        Self::rotate_bitmap(bitmap)
                    }
                };

                self.draw(&bitmap).await.terminal_err()?;

                // Increment character counter and potentially wrap line
                self.advance_cursor().await?;
            }
        }

        Ok(())
    }

    /// Get the current cursor position, in character coordinates.
    /// This is the (column, row) that the next character will be written to.
    pub fn position(&self) -> Result<(u8, u8), TerminalModeError> {
        self.mode
            .cursor
            .as_ref()
            .map(|c| c.get_position())
            .ok_or(TerminalModeError::Uninitialized)
    }

    /// Set the cursor position, in character coordinates.
    /// This is the (column, row) that the next character will be written to.
    /// If the position is out of bounds, an Err will be returned.
    pub async fn set_position(&mut self, column: u8, row: u8) -> Result<(), TerminalModeError> {
        let (width, height) = self.ensure_cursor()?.get_dimensions();
        if column >= width || row >= height {
            Err(TerminalModeError::OutOfBounds)
        } else {
            let offset_x = match self.rotation() {
                DisplayRotation::Rotate0 | DisplayRotation::Rotate270 => SIZE::OFFSETX,
                DisplayRotation::Rotate180 | DisplayRotation::Rotate90 => {
                    // If segment remapping is flipped, we need to calculate
                    // the offset from the other edge of the display.
                    SIZE::DRIVER_COLS - SIZE::WIDTH - SIZE::OFFSETX
                }
            };
            match self.rotation() {
                DisplayRotation::Rotate0 | DisplayRotation::Rotate180 => {
                    self.set_column(offset_x + column * 8)
                        .await
                        .terminal_err()?;
                    self.set_row(SIZE::OFFSETY + row * 8).await.terminal_err()?;
                }
                DisplayRotation::Rotate90 | DisplayRotation::Rotate270 => {
                    self.set_column(offset_x + row * 8).await.terminal_err()?;
                    self.set_row(SIZE::OFFSETY + column * 8)
                        .await
                        .terminal_err()?;
                }
            }
            self.ensure_cursor()?.set_position(column, row);
            Ok(())
        }
    }

    /// Reset the draw area and move pointer to the top left corner
    async fn reset_pos(&mut self) -> Result<(), TerminalModeError> {
        // Initialise the counter when we know it's valid
        let (w, h) = match self.rotation() {
            DisplayRotation::Rotate0 | DisplayRotation::Rotate180 => (SIZE::WIDTH, SIZE::HEIGHT),
            DisplayRotation::Rotate90 | DisplayRotation::Rotate270 => (SIZE::HEIGHT, SIZE::WIDTH),
        };
        self.mode.cursor = Some(Cursor::new(w, h));

        // Reset cursor position
        self.set_position(0, 0).await?;

        Ok(())
    }

    /// Advance the cursor, automatically wrapping lines and/or screens if necessary
    /// Takes in an already-unwrapped cursor to avoid re-unwrapping
    async fn advance_cursor(&mut self) -> Result<(), TerminalModeError> {
        let cursor = self.ensure_cursor()?;

        cursor.advance();
        let (c, r) = cursor.get_position();
        self.set_position(c, r).await?;

        Ok(())
    }

    fn ensure_cursor(&mut self) -> Result<&mut Cursor, TerminalModeError> {
        self.mode
            .cursor
            .as_mut()
            .ok_or(TerminalModeError::Uninitialized)
    }

    fn char_to_bitmap(input: char) -> [u8; 8] {
        // Populate the array with the data from the character array at the right index
        match input {
            '!' => [0x00, 0x00, 0x2f, 0x00, 0x00, 0x00, 0x00, 0x00],
            '"' => [0x00, 0x03, 0x00, 0x03, 0x00, 0x00, 0x00, 0x00],
            '#' => [0x00, 0x12, 0x3f, 0x12, 0x12, 0x3f, 0x12, 0x00],
            '$' => [0x00, 0x2e, 0x2a, 0x7f, 0x2a, 0x3a, 0x00, 0x00],
            '%' => [0x00, 0x23, 0x13, 0x08, 0x04, 0x32, 0x31, 0x00],
            '&' => [0x00, 0x10, 0x2a, 0x25, 0x2a, 0x10, 0x20, 0x00],
            '\'' => [0x00, 0x02, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00],
            '(' => [0x00, 0x1e, 0x21, 0x00, 0x00, 0x00, 0x00, 0x00],
            ')' => [0x00, 0x21, 0x1e, 0x00, 0x00, 0x00, 0x00, 0x00],
            '*' => [0x00, 0x08, 0x2a, 0x1c, 0x2a, 0x08, 0x00, 0x00],
            '+' => [0x00, 0x08, 0x08, 0x3e, 0x08, 0x08, 0x00, 0x00],
            ',' => [0x00, 0x80, 0x60, 0x00, 0x00, 0x00, 0x00, 0x00],
            '-' => [0x00, 0x08, 0x08, 0x08, 0x08, 0x08, 0x00, 0x00],
            '.' => [0x00, 0x30, 0x30, 0x00, 0x00, 0x00, 0x00, 0x00],
            '/' => [0x00, 0x20, 0x10, 0x08, 0x04, 0x02, 0x00, 0x00],
            '0' => [0x00, 0x1e, 0x31, 0x29, 0x25, 0x23, 0x1e, 0x00],
            '1' => [0x00, 0x22, 0x21, 0x3f, 0x20, 0x20, 0x20, 0x00],
            '2' => [0x00, 0x32, 0x29, 0x29, 0x29, 0x29, 0x26, 0x00],
            '3' => [0x00, 0x12, 0x21, 0x21, 0x25, 0x25, 0x1a, 0x00],
            '4' => [0x00, 0x18, 0x14, 0x12, 0x3f, 0x10, 0x00, 0x00],
            '5' => [0x00, 0x17, 0x25, 0x25, 0x25, 0x25, 0x19, 0x00],
            '6' => [0x00, 0x1e, 0x25, 0x25, 0x25, 0x25, 0x18, 0x00],
            '7' => [0x00, 0x01, 0x01, 0x31, 0x09, 0x05, 0x03, 0x00],
            '8' => [0x00, 0x1a, 0x25, 0x25, 0x25, 0x25, 0x1a, 0x00],
            '9' => [0x00, 0x06, 0x29, 0x29, 0x29, 0x29, 0x1e, 0x00],
            ':' => [0x00, 0x24, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
            ';' => [0x00, 0x80, 0x64, 0x00, 0x00, 0x00, 0x00, 0x00],
            '<' => [0x00, 0x08, 0x14, 0x22, 0x00, 0x00, 0x00, 0x00],
            '=' => [0x00, 0x14, 0x14, 0x14, 0x14, 0x14, 0x00, 0x00],
            '>' => [0x00, 0x22, 0x14, 0x08, 0x00, 0x00, 0x00, 0x00],
            '?' => [0x00, 0x02, 0x01, 0x01, 0x29, 0x05, 0x02, 0x00],
            '@' => [0x00, 0x1e, 0x21, 0x2d, 0x2b, 0x2d, 0x0e, 0x00],
            'A' => [0x00, 0x3e, 0x09, 0x09, 0x09, 0x09, 0x3e, 0x00],
            'B' => [0x00, 0x3f, 0x25, 0x25, 0x25, 0x25, 0x1a, 0x00],
            'C' => [0x00, 0x1e, 0x21, 0x21, 0x21, 0x21, 0x12, 0x00],
            'D' => [0x00, 0x3f, 0x21, 0x21, 0x21, 0x12, 0x0c, 0x00],
            'E' => [0x00, 0x3f, 0x25, 0x25, 0x25, 0x25, 0x21, 0x00],
            'F' => [0x00, 0x3f, 0x05, 0x05, 0x05, 0x05, 0x01, 0x00],
            'G' => [0x00, 0x1e, 0x21, 0x21, 0x21, 0x29, 0x1a, 0x00],
            'H' => [0x00, 0x3f, 0x04, 0x04, 0x04, 0x04, 0x3f, 0x00],
            'I' => [0x00, 0x21, 0x21, 0x3f, 0x21, 0x21, 0x00, 0x00],
            'J' => [0x00, 0x10, 0x20, 0x20, 0x20, 0x20, 0x1f, 0x00],
            'K' => [0x00, 0x3f, 0x04, 0x0c, 0x0a, 0x11, 0x20, 0x00],
            'L' => [0x00, 0x3f, 0x20, 0x20, 0x20, 0x20, 0x20, 0x00],
            'M' => [0x00, 0x3f, 0x02, 0x04, 0x04, 0x02, 0x3f, 0x00],
            'N' => [0x00, 0x3f, 0x02, 0x04, 0x08, 0x10, 0x3f, 0x00],
            'O' => [0x00, 0x1e, 0x21, 0x21, 0x21, 0x21, 0x1e, 0x00],
            'P' => [0x00, 0x3f, 0x09, 0x09, 0x09, 0x09, 0x06, 0x00],
            'Q' => [0x00, 0x1e, 0x21, 0x29, 0x31, 0x21, 0x5e, 0x00],
            'R' => [0x00, 0x3f, 0x09, 0x09, 0x09, 0x19, 0x26, 0x00],
            'S' => [0x00, 0x12, 0x25, 0x25, 0x25, 0x25, 0x18, 0x00],
            'T' => [0x00, 0x01, 0x01, 0x01, 0x3f, 0x01, 0x01, 0x00],
            'U' => [0x00, 0x1f, 0x20, 0x20, 0x20, 0x20, 0x1f, 0x00],
            'V' => [0x00, 0x0f, 0x10, 0x20, 0x20, 0x10, 0x0f, 0x00],
            'W' => [0x00, 0x1f, 0x20, 0x10, 0x10, 0x20, 0x1f, 0x00],
            'X' => [0x00, 0x21, 0x12, 0x0c, 0x0c, 0x12, 0x21, 0x00],
            'Y' => [0x00, 0x01, 0x02, 0x3c, 0x02, 0x01, 0x00, 0x00],
            'Z' => [0x00, 0x21, 0x31, 0x29, 0x25, 0x23, 0x21, 0x00],
            '[' => [0x00, 0x3f, 0x21, 0x00, 0x00, 0x00, 0x00, 0x00],
            '\\' => [0x00, 0x02, 0x04, 0x08, 0x10, 0x20, 0x00, 0x00],
            ']' => [0x00, 0x21, 0x3f, 0x00, 0x00, 0x00, 0x00, 0x00],
            '^' => [0x00, 0x04, 0x02, 0x3f, 0x02, 0x04, 0x00, 0x00],
            '_' => [0x00, 0x40, 0x40, 0x40, 0x40, 0x40, 0x40, 0x00],
            '`' => [0x00, 0x01, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00],
            'a' => [0x00, 0x10, 0x2a, 0x2a, 0x2a, 0x3c, 0x00, 0x00],
            'b' => [0x00, 0x3f, 0x24, 0x24, 0x24, 0x18, 0x00, 0x00],
            'c' => [0x00, 0x1c, 0x22, 0x22, 0x22, 0x00, 0x00, 0x00],
            'd' => [0x00, 0x18, 0x24, 0x24, 0x24, 0x3f, 0x00, 0x00],
            'e' => [0x00, 0x1c, 0x2a, 0x2a, 0x2a, 0x24, 0x00, 0x00],
            'f' => [0x00, 0x00, 0x3e, 0x05, 0x01, 0x00, 0x00, 0x00],
            'g' => [0x00, 0x18, 0xa4, 0xa4, 0xa4, 0x7c, 0x00, 0x00],
            'h' => [0x00, 0x3f, 0x04, 0x04, 0x04, 0x38, 0x00, 0x00],
            'i' => [0x00, 0x00, 0x24, 0x3d, 0x20, 0x00, 0x00, 0x00],
            'j' => [0x00, 0x20, 0x40, 0x40, 0x3d, 0x00, 0x00, 0x00],
            'k' => [0x00, 0x3f, 0x0c, 0x12, 0x20, 0x00, 0x00, 0x00],
            'l' => [0x00, 0x1f, 0x20, 0x20, 0x00, 0x00, 0x00, 0x00],
            'm' => [0x00, 0x3e, 0x02, 0x3c, 0x02, 0x3c, 0x00, 0x00],
            'n' => [0x00, 0x3e, 0x02, 0x02, 0x02, 0x3c, 0x00, 0x00],
            'o' => [0x00, 0x1c, 0x22, 0x22, 0x22, 0x1c, 0x00, 0x00],
            'p' => [0x00, 0xfc, 0x24, 0x24, 0x24, 0x18, 0x00, 0x00],
            'q' => [0x00, 0x18, 0x24, 0x24, 0x24, 0xfc, 0x00, 0x00],
            'r' => [0x00, 0x3e, 0x04, 0x02, 0x02, 0x00, 0x00, 0x00],
            's' => [0x00, 0x24, 0x2a, 0x2a, 0x2a, 0x10, 0x00, 0x00],
            't' => [0x00, 0x02, 0x1f, 0x22, 0x20, 0x00, 0x00, 0x00],
            'u' => [0x00, 0x1e, 0x20, 0x20, 0x20, 0x1e, 0x00, 0x00],
            'v' => [0x00, 0x06, 0x18, 0x20, 0x18, 0x06, 0x00, 0x00],
            'w' => [0x00, 0x1e, 0x30, 0x1c, 0x30, 0x1e, 0x00, 0x00],
            'x' => [0x00, 0x22, 0x14, 0x08, 0x14, 0x22, 0x00, 0x00],
            'y' => [0x00, 0x1c, 0xa0, 0xa0, 0xa0, 0x7c, 0x00, 0x00],
            'z' => [0x00, 0x22, 0x32, 0x2a, 0x26, 0x22, 0x00, 0x00],
            '{' => [0x00, 0x0c, 0x3f, 0x21, 0x00, 0x00, 0x00, 0x00],
            '|' => [0x00, 0x3f, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
            '}' => [0x00, 0x21, 0x3f, 0x0c, 0x00, 0x00, 0x00, 0x00],
            '~' => [0x00, 0x02, 0x01, 0x02, 0x01, 0x00, 0x00, 0x00],
            _ => [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
        }
    }

    fn rotate_bitmap(bitmap: [u8; 8]) -> [u8; 8] {
        let mut rotated: [u8; 8] = [0; 8];

        for col in 0..8 {
            // source.msb is the top pixel
            let source = bitmap[col];
            for row in 0..8 {
                let bit = source & 1 << row != 0;
                if bit {
                    rotated[row] |= 1 << col;
                }
            }
        }

        rotated
    }
}

use embassy_futures::block_on;
impl<DI, SIZE> core::fmt::Write for Ssd1306<DI, SIZE, TerminalMode>
where
    DI: WriteOnlyDataCommand<Error = DisplayError>,
    SIZE: TerminalDisplaySize,
{
    fn write_str(&mut self, s: &str) -> Result<(), fmt::Error> {
        s.chars().map(move |c| block_on(self.print_char(c))).last();
        Ok(())
    }
}
