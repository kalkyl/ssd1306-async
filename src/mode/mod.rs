//! Display modes.

mod buffered_graphics;
mod terminal;

use crate::{command::AddrMode, rotation::DisplayRotation, size::DisplaySize, Ssd1306};
use crate::{DisplayError, WriteOnlyDataCommand};
pub use buffered_graphics::*;
use core::future::Future;
pub use terminal::*;

/// Common functions to all display modes.
pub trait DisplayConfig {
    /// Error.
    type Error;
    type WriteFuture<'a>: Future<Output = Result<(), Self::Error>>
    where
        Self: 'a;
    type InitFuture<'a>: Future<Output = Result<(), Self::Error>>
    where
        Self: 'a;

    /// Set display rotation.
    fn set_rotation<'a>(&'a mut self, rotation: DisplayRotation) -> Self::WriteFuture<'a>;

    /// Initialise and configure the display for the given mode.
    fn init<'a>(&'a mut self) -> Self::InitFuture<'a>;
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
    type WriteFuture<'a> = impl Future<Output = Result<(), Self::Error>> + 'a where Self: 'a;
    /// Set the display rotation.
    fn set_rotation<'a>(&'a mut self, rot: DisplayRotation) -> Self::WriteFuture<'a> {
        async move {
            self.set_rotation(rot).await?;
            Ok(())
        }
    }

    type InitFuture<'a> = impl Future<Output = Result<(), Self::Error>> + 'a where Self: 'a;
    /// Initialise in horizontal addressing mode.
    fn init<'a>(&'a mut self) -> Self::InitFuture<'a> {
        async move {
            self.init_with_addr_mode(AddrMode::Horizontal).await?;
            Ok(())
        }
    }
}
