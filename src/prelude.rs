//! Crate prelude

pub use crate::WriteOnlyDataCommand;
// pub use display_interface_i2c::I2CInterface;
// pub use display_interface_spi::{SPIInterface, SPIInterfaceNoCS};
pub use crate::i2c_interface::I2CDisplayInterface;
pub use crate::spi_interface::{SPIInterface, SPIInterfaceNoCS};

pub use super::{
    brightness::Brightness,
    mode::DisplayConfig,
    rotation::DisplayRotation,
    size::{
        DisplaySize, DisplaySize128x32, DisplaySize128x64, DisplaySize64x48, DisplaySize72x40,
        DisplaySize96x16,
    },
};
