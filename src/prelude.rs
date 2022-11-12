//! Crate prelude

pub use crate::i2c_interface::I2CDisplayInterface;
pub use crate::spi_interface::SPIInterface;
pub use crate::WriteOnlyDataCommand;

pub use super::{
    brightness::Brightness,
    mode::DisplayConfig,
    rotation::DisplayRotation,
    size::{
        DisplaySize, DisplaySize128x32, DisplaySize128x64, DisplaySize64x48, DisplaySize72x40,
        DisplaySize96x16,
    },
};
