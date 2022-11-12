//! SSD1306 OLED display driver.
//!
//! This crate provides a driver interface to the popular SSD1306 monochrome OLED display driver. It
//! supports I2C and SPI via `embedded-hal-async` traits.
//!
//! The main driver is created using [`Ssd1306::new`] which accepts an interface instance, display
//! size, rotation and mode. The following display modes are supported:
//!
//! - [`BasicMode`](crate::mode::BasicMode) - A simple mode with lower level methods available.
//! - [`BufferedGraphicsMode`] - A framebuffered mode with additional methods and integration with
//!   [embedded-graphics](https://docs.rs/embedded-graphics).
//! - [`TerminalMode`] - A bufferless mode supporting drawing text to the display, as well as
//!   setting cursor positions like a simple terminal.
//!
//! # Examples
//!
//! Examples can be found in [the examples/
//! folder](https://github.com/kalkyl/ssd1306-async/blob/main/examples)
//!
//! [featureset]: https://github.com/jamwaffles/embedded-graphics#features
//! [`BufferedGraphicsMode`]: crate::mode::BufferedGraphicsMode
//! [`TerminalMode`]: crate::mode::TerminalMode

#![no_std]
#![feature(type_alias_impl_trait)]
// #![deny(missing_debug_implementations)]
// #![deny(missing_docs)]
// #![deny(warnings)]
// #![deny(missing_copy_implementations)]
// #![deny(trivial_casts)]
// #![deny(trivial_numeric_casts)]
// #![deny(unsafe_code)]
// #![deny(unstable_features)]
// #![deny(unused_import_braces)]
// #![deny(unused_qualifications)]
// #![deny(rustdoc::broken_intra_doc_links)]

mod brightness;
pub mod command;
mod error;
mod i2c_interface;
pub mod mode;
pub mod prelude;
pub mod rotation;
pub mod size;
mod spi_interface;

pub use crate::i2c_interface::I2CDisplayInterface;
use crate::mode::BasicMode;
pub use crate::spi_interface::SPIInterface;
use crate::DataFormat::U8;
use brightness::Brightness;
use command::{AddrMode, Command, VcomhLevel};
use core::future::Future;
use embedded_hal::{blocking::delay::DelayMs, digital::v2::OutputPin};
use error::Error;
use mode::{BufferedGraphicsMode, TerminalMode};
use rotation::DisplayRotation;
use size::DisplaySize;

/// SSD1306 driver.
///
/// Note that some methods are only available when the display is configured in a certain [`mode`].
#[derive(Copy, Clone, Debug)]
pub struct Ssd1306<DI, SIZE, MODE> {
    interface: DI,
    mode: MODE,
    size: SIZE,
    addr_mode: AddrMode,
    rotation: DisplayRotation,
}

impl<DI, SIZE> Ssd1306<DI, SIZE, BasicMode>
where
    DI: WriteOnlyDataCommand,
    SIZE: DisplaySize,
{
    /// Create a basic SSD1306 interface.
    ///
    /// Use the `into_*_mode` methods to enable more functionality.
    pub fn new(interface: DI, size: SIZE, rotation: DisplayRotation) -> Self {
        Self {
            interface,
            size,
            addr_mode: AddrMode::Page,
            mode: BasicMode,
            rotation,
        }
    }
}

impl<DI, SIZE, MODE> Ssd1306<DI, SIZE, MODE>
where
    DI: WriteOnlyDataCommand<Error = DisplayError>,
    SIZE: DisplaySize,
{
    /// Convert the display into another interface mode.
    fn into_mode<MODE2>(self, mode: MODE2) -> Ssd1306<DI, SIZE, MODE2> {
        Ssd1306 {
            mode,
            addr_mode: self.addr_mode,
            interface: self.interface,
            size: self.size,
            rotation: self.rotation,
        }
    }

    /// Convert the display into a buffered graphics mode, supporting
    /// [embedded-graphics](https://crates.io/crates/embedded-graphics).
    ///
    /// See [BufferedGraphicsMode] for more information.
    pub fn into_buffered_graphics_mode(self) -> Ssd1306<DI, SIZE, BufferedGraphicsMode<SIZE>> {
        self.into_mode(BufferedGraphicsMode::new())
    }

    /// Convert the display into a text-only, terminal-like mode.
    ///
    /// See [TerminalMode] for more information.
    pub fn into_terminal_mode(self) -> Ssd1306<DI, SIZE, TerminalMode> {
        self.into_mode(TerminalMode::new())
    }

    /// Initialise the display in one of the available addressing modes.
    pub async fn init_with_addr_mode(&mut self, mode: AddrMode) -> Result<(), DisplayError> {
        let rotation = self.rotation;

        Command::DisplayOn(false).send(&mut self.interface).await?;
        Command::DisplayClockDiv(0x8, 0x0)
            .send(&mut self.interface)
            .await?;
        Command::Multiplex(SIZE::HEIGHT - 1)
            .send(&mut self.interface)
            .await?;
        Command::DisplayOffset(0).send(&mut self.interface).await?;
        Command::StartLine(0).send(&mut self.interface).await?;
        // TODO: Ability to turn charge pump on/off
        Command::ChargePump(true).send(&mut self.interface).await?;
        Command::AddressMode(mode).send(&mut self.interface).await?;

        self.size.com_pin_cfg().send(&mut self.interface).await?;
        if let Some(cmd) = self.size.int_iref() {
            cmd.send(&mut self.interface).await?
        }
        self.set_rotation(rotation).await?;

        self.set_brightness(Brightness::default()).await?;
        Command::VcomhDeselect(VcomhLevel::Auto)
            .send(&mut self.interface)
            .await?;
        Command::AllOn(false).send(&mut self.interface).await?;
        Command::Invert(false).send(&mut self.interface).await?;
        Command::EnableScroll(false)
            .send(&mut self.interface)
            .await?;
        Command::DisplayOn(true).send(&mut self.interface).await?;

        self.addr_mode = mode;

        Ok(())
    }

    /// Change the addressing mode
    pub async fn set_addr_mode(&mut self, mode: AddrMode) -> Result<(), DisplayError> {
        Command::AddressMode(mode).send(&mut self.interface).await?;
        self.addr_mode = mode;
        Ok(())
    }

    /// Send the data to the display for drawing at the current position in the framebuffer
    /// and advance the position accordingly. Cf. `set_draw_area` to modify the affected area by
    /// this method.
    ///
    /// This method takes advantage of a bounding box for faster writes.
    pub async fn bounded_draw(
        &mut self,
        buffer: &[u8],
        disp_width: usize,
        upper_left: (u8, u8),
        lower_right: (u8, u8),
    ) -> Result<(), DisplayError> {
        Self::flush_buffer_chunks(
            &mut self.interface,
            buffer,
            disp_width,
            upper_left,
            lower_right,
        )
        .await
    }

    /// Send a raw buffer to the display.
    pub async fn draw(&mut self, buffer: &[u8]) -> Result<(), DisplayError> {
        self.interface.send_data(U8(&buffer)).await
    }

    /// Get display dimensions, taking into account the current rotation of the display
    ///
    /// ```rust
    /// # use ssd1306::test_helpers::StubInterface;
    /// # let interface = StubInterface;
    /// use ssd1306::{mode::TerminalMode, prelude::*, Ssd1306};
    ///
    /// let mut display = Ssd1306::new(
    ///     interface,
    ///     DisplaySize128x64,
    ///     DisplayRotation::Rotate0,
    /// ).into_terminal_mode();
    /// assert_eq!(display.dimensions(), (128, 64));
    ///
    /// # let interface = StubInterface;
    /// let mut rotated_display = Ssd1306::new(
    ///     interface,
    ///     DisplaySize128x64,
    ///     DisplayRotation::Rotate90,
    /// ).into_terminal_mode();
    /// assert_eq!(rotated_display.dimensions(), (64, 128));
    /// ```
    pub fn dimensions(&self) -> (u8, u8) {
        match self.rotation {
            DisplayRotation::Rotate0 | DisplayRotation::Rotate180 => (SIZE::WIDTH, SIZE::HEIGHT),
            DisplayRotation::Rotate90 | DisplayRotation::Rotate270 => (SIZE::HEIGHT, SIZE::WIDTH),
        }
    }

    /// Get the display rotation.
    pub fn rotation(&self) -> DisplayRotation {
        self.rotation
    }

    /// Set the display rotation.
    pub async fn set_rotation(&mut self, rotation: DisplayRotation) -> Result<(), DisplayError> {
        self.rotation = rotation;

        match rotation {
            DisplayRotation::Rotate0 => {
                Command::SegmentRemap(true)
                    .send(&mut self.interface)
                    .await?;
                Command::ReverseComDir(true)
                    .send(&mut self.interface)
                    .await?;
            }
            DisplayRotation::Rotate90 => {
                Command::SegmentRemap(false)
                    .send(&mut self.interface)
                    .await?;
                Command::ReverseComDir(true)
                    .send(&mut self.interface)
                    .await?;
            }
            DisplayRotation::Rotate180 => {
                Command::SegmentRemap(false)
                    .send(&mut self.interface)
                    .await?;
                Command::ReverseComDir(false)
                    .send(&mut self.interface)
                    .await?;
            }
            DisplayRotation::Rotate270 => {
                Command::SegmentRemap(true)
                    .send(&mut self.interface)
                    .await?;
                Command::ReverseComDir(false)
                    .send(&mut self.interface)
                    .await?;
            }
        };

        Ok(())
    }

    /// Set mirror enabled/disabled.
    pub async fn set_mirror(&mut self, mirror: bool) -> Result<(), DisplayError> {
        if mirror {
            match self.rotation {
                DisplayRotation::Rotate0 => {
                    Command::SegmentRemap(false)
                        .send(&mut self.interface)
                        .await?;
                    Command::ReverseComDir(true)
                        .send(&mut self.interface)
                        .await?;
                }
                DisplayRotation::Rotate90 => {
                    Command::SegmentRemap(false)
                        .send(&mut self.interface)
                        .await?;
                    Command::ReverseComDir(false)
                        .send(&mut self.interface)
                        .await?;
                }
                DisplayRotation::Rotate180 => {
                    Command::SegmentRemap(true)
                        .send(&mut self.interface)
                        .await?;
                    Command::ReverseComDir(false)
                        .send(&mut self.interface)
                        .await?;
                }
                DisplayRotation::Rotate270 => {
                    Command::SegmentRemap(true)
                        .send(&mut self.interface)
                        .await?;
                    Command::ReverseComDir(true)
                        .send(&mut self.interface)
                        .await?;
                }
            };
        } else {
            self.set_rotation(self.rotation).await?;
        }
        Ok(())
    }

    /// Change the display brightness.
    pub async fn set_brightness(&mut self, brightness: Brightness) -> Result<(), DisplayError> {
        // Should be moved to Brightness::new once conditions can be used in const functions
        debug_assert!(
            0 < brightness.precharge && brightness.precharge <= 15,
            "Precharge value must be between 1 and 15"
        );

        Command::PreChargePeriod(1, brightness.precharge)
            .send(&mut self.interface)
            .await?;
        Command::Contrast(brightness.contrast)
            .send(&mut self.interface)
            .await
    }

    /// Turn the display on or off. The display can be drawn to and retains all
    /// of its memory even while off.
    pub async fn set_display_on(&mut self, on: bool) -> Result<(), DisplayError> {
        Command::DisplayOn(on).send(&mut self.interface).await
    }

    /// Set the position in the framebuffer of the display limiting where any sent data should be
    /// drawn. This method can be used for changing the affected area on the screen as well
    /// as (re-)setting the start point of the next `draw` call.
    pub async fn set_draw_area(
        &mut self,
        start: (u8, u8),
        end: (u8, u8),
    ) -> Result<(), DisplayError> {
        Command::ColumnAddress(start.0, end.0.saturating_sub(1))
            .send(&mut self.interface)
            .await?;

        if self.addr_mode != AddrMode::Page {
            Command::PageAddress(start.1.into(), (end.1.saturating_sub(1)).into())
                .send(&mut self.interface)
                .await?;
        }

        Ok(())
    }

    /// Set the column address in the framebuffer of the display where any sent data should be
    /// drawn.
    pub async fn set_column(&mut self, column: u8) -> Result<(), DisplayError> {
        Command::ColStart(column).send(&mut self.interface).await
    }

    /// Set the page address (row 8px high) in the framebuffer of the display where any sent data
    /// should be drawn.
    ///
    /// Note that the parameter is in pixels, but the page will be set to the start of the 8px
    /// row which contains the passed-in row.
    pub async fn set_row(&mut self, row: u8) -> Result<(), DisplayError> {
        Command::PageStart(row.into())
            .send(&mut self.interface)
            .await
    }

    async fn flush_buffer_chunks(
        interface: &mut DI,
        buffer: &[u8],
        disp_width: usize,
        upper_left: (u8, u8),
        lower_right: (u8, u8),
    ) -> Result<(), DisplayError> {
        // Divide by 8 since each row is actually 8 pixels tall
        let num_pages = ((lower_right.1 - upper_left.1) / 8) as usize + 1;

        // Each page is 8 bits tall, so calculate which page number to start at (rounded down) from
        // the top of the display
        let starting_page = (upper_left.1 / 8) as usize;

        // Calculate start and end X coordinates for each page
        let page_lower = upper_left.0 as usize;
        let page_upper = lower_right.0 as usize;

        for c in buffer
            .chunks(disp_width)
            .skip(starting_page)
            .take(num_pages)
            .map(|s| &s[page_lower..page_upper])
        {
            interface.send_data(U8(&c)).await?;
        }
        Ok(())
    }
}

// SPI-only reset
impl<SPI, DC, SIZE, MODE> Ssd1306<SPIInterface<SPI, DC>, SIZE, MODE> {
    /// Reset the display.
    pub fn reset<RST, DELAY, PinE>(
        &mut self,
        rst: &mut RST,
        delay: &mut DELAY,
    ) -> Result<(), Error<(), PinE>>
    where
        RST: OutputPin<Error = PinE>,
        DELAY: DelayMs<u8>,
    {
        inner_reset(rst, delay)
    }
}

fn inner_reset<RST, DELAY, PinE>(rst: &mut RST, delay: &mut DELAY) -> Result<(), Error<(), PinE>>
where
    RST: OutputPin<Error = PinE>,
    DELAY: DelayMs<u8>,
{
    rst.set_high().map_err(Error::Pin)?;
    delay.delay_ms(1);
    rst.set_low().map_err(Error::Pin)?;
    delay.delay_ms(10);
    rst.set_high().map_err(Error::Pin)
}

/// A ubiquitous error type for all kinds of problems which could happen when communicating with a
/// display
#[derive(Clone, Debug)]
#[non_exhaustive]
pub enum DisplayError {
    /// Invalid data format selected for interface selected
    InvalidFormatError,
    /// Unable to write to bus
    BusWriteError,
    /// Unable to assert or de-assert data/command switching signal
    DCError,
    /// Unable to assert chip select signal
    CSError,
    /// The requested DataFormat is not implemented by this display interface implementation
    DataFormatNotImplemented,
    /// Unable to assert or de-assert reset signal
    RSError,
    /// Attempted to write to a non-existing pixel outside the display's bounds
    OutOfBoundsError,
}

/// DI specific data format wrapper around slices of various widths
/// Display drivers need to implement non-trivial conversions (e.g. with padding)
/// as the hardware requires.
#[non_exhaustive]
pub enum DataFormat<'a> {
    /// Slice of unsigned bytes
    U8(&'a [u8]),
    /// Slice of unsigned 16bit values with the same endianess as the system, not recommended
    U16(&'a [u16]),
    /// Slice of unsigned 16bit values to be sent in big endian byte order
    U16BE(&'a mut [u16]),
    /// Slice of unsigned 16bit values to be sent in little endian byte order
    U16LE(&'a mut [u16]),
    /// Iterator over unsigned bytes
    U8Iter(&'a mut dyn Iterator<Item = u8>),
    /// Iterator over unsigned 16bit values to be sent in big endian byte order
    U16BEIter(&'a mut dyn Iterator<Item = u16>),
    /// Iterator over unsigned 16bit values to be sent in little endian byte order
    U16LEIter(&'a mut dyn Iterator<Item = u16>),
}

/// This trait implements a write-only interface for a display which has separate data and command
/// modes. It is the responsibility of implementations to activate the correct mode in their
/// implementation when corresponding method is called.
pub trait WriteOnlyDataCommand {
    type Error;
    type WriteFuture<'a>: Future<Output = Result<(), Self::Error>>
    where
        Self: 'a;
    type DataFuture<'a>: Future<Output = Result<(), Self::Error>>
    where
        Self: 'a;

    /// Send a batch of commands to display
    fn send_commands<'a>(&'a mut self, cmd: DataFormat<'a>) -> Self::WriteFuture<'a>;

    /// Send pixel data to display
    fn send_data<'a>(&'a mut self, buf: DataFormat<'a>) -> Self::DataFuture<'a>;
}
