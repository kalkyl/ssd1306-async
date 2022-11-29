//! Display modes.

mod buffered_graphics;
mod terminal;

use crate::{command::AddrMode, rotation::DisplayRotation, size::DisplaySize, Ssd1306};
use crate::{DisplayError, WriteOnlyDataCommand};
pub use buffered_graphics::*;
pub use terminal::*;

/// Common functions to all display modes.
pub trait DisplayConfig {
    /// Error.
    type Error;

    /// Set display rotation.
    async fn set_rotation(&mut self, rotation: DisplayRotation) -> Result<(), Self::Error>;

    /// Initialise and configure the display for the given mode.
    async fn init(&mut self) -> Result<(), Self::Error>;
}

/// A mode with no additional functionality beyond that provided by the base [`Ssd1306`] struct.
#[derive(Debug, Copy, Clone)]
pub struct BasicMode;

impl<DI, SIZE> Ssd1306<DI, SIZE, BasicMode>
where
    DI: WriteOnlyDataCommand<Error = DisplayError>,
    SIZE: DisplaySize,
{
    /// Clear the display.
    pub async fn clear(&mut self) -> Result<(), DisplayError> {
        self.set_draw_area((0, 0), self.dimensions()).await?;

        // TODO: If const generics allows this, replace `1024` with computed W x H value for current
        // `SIZE`.
        self.draw(&[0u8; 1024]).await
    }
}

impl<DI, SIZE> DisplayConfig for Ssd1306<DI, SIZE, BasicMode>
where
    DI: WriteOnlyDataCommand<Error = DisplayError>,
    SIZE: DisplaySize,
{
    type Error = DisplayError;

    /// Set the display rotation.
    async fn set_rotation(&mut self, rot: DisplayRotation) -> Result<(), Self::Error> {
        self.set_rotation(rot).await?;
        Ok(())
    }

    /// Initialise in horizontal addressing mode.
    async fn init(&mut self) -> Result<(), Self::Error> {
        self.init_with_addr_mode(AddrMode::Horizontal).await?;
        Ok(())
    }
}
