//! I2C interface factory

use crate::{DataFormat, DisplayError, WriteOnlyDataCommand};
use core::future::Future;
use embedded_hal_async as hal;

/// Helper struct to create preconfigured I2C interfaces for the display.
#[derive(Debug, Copy, Clone)]
pub struct I2CDisplayInterface(());

impl I2CDisplayInterface {
    /// Create new builder with a default I2C address of 0x3C
    pub fn new<I>(i2c: I) -> I2CInterface<I>
    where
        I: hal::i2c::I2c,
    {
        Self::new_custom_address(i2c, 0x3C)
    }

    /// Create a new I2C interface with the alternate address 0x3D as specified in the datasheet.
    pub fn new_alternate_address<I>(i2c: I) -> I2CInterface<I>
    where
        I: hal::i2c::I2c,
    {
        Self::new_custom_address(i2c, 0x3D)
    }

    /// Create a new I2C interface with a custom address.
    pub fn new_custom_address<I>(i2c: I, address: u8) -> I2CInterface<I>
    where
        I: hal::i2c::I2c,
    {
        I2CInterface::new(i2c, address, 0x40)
    }
}

/// I2C communication interface
pub struct I2CInterface<I2C> {
    i2c: I2C,
    addr: u8,
    data_byte: u8,
}

impl<I2C> I2CInterface<I2C>
where
    I2C: hal::i2c::I2c,
{
    /// Create new I2C interface for communication with a display driver
    pub fn new(i2c: I2C, addr: u8, data_byte: u8) -> Self {
        Self {
            i2c,
            addr,
            data_byte,
        }
    }

    /// Consume the display interface and return
    /// the underlying peripherial driver
    pub fn release(self) -> I2C {
        self.i2c
    }
}

impl<I2C> WriteOnlyDataCommand for I2CInterface<I2C>
where
    I2C: hal::i2c::I2c,
{
    type Error = DisplayError;
    type WriteFuture<'a> = impl Future<Output = Result<(), Self::Error>> + 'a where Self: 'a;

    fn send_commands<'a>(&'a mut self, cmds: DataFormat<'a>) -> Self::WriteFuture<'a> {
        async move {
            // Copy over given commands to new aray to prefix with command identifier
            match cmds {
                DataFormat::U8(slice) => {
                    let mut writebuf: [u8; 8] = [0; 8];
                    writebuf[1..=slice.len()].copy_from_slice(&slice[0..slice.len()]);

                    self.i2c
                        .write(self.addr, &writebuf[..=slice.len()])
                        .await
                        .map_err(|_| DisplayError::BusWriteError)?;
                    Ok(())
                }
                _ => Err(DisplayError::DataFormatNotImplemented),
            }
        }
    }

    type DataFuture<'a> = impl Future<Output = Result<(), Self::Error>> + 'a where Self: 'a;
    fn send_data<'a>(&'a mut self, buf: DataFormat<'a>) -> Self::DataFuture<'a> {
        async move {
            match buf {
                DataFormat::U8(slice) => {
                    // No-op if the data buffer is empty
                    if slice.is_empty() {
                        return Ok(());
                    }

                    let mut writebuf = [0; 17];

                    // Data mode
                    writebuf[0] = self.data_byte;

                    for c in slice.chunks(16) {
                        let chunk_len = c.len();

                        // Copy over all data from buffer, leaving the data command byte intact
                        writebuf[1..=chunk_len].copy_from_slice(c);

                        self.i2c
                            .write(self.addr, &writebuf[0..=chunk_len])
                            .await
                            .map_err(|_| DisplayError::BusWriteError)?;
                    }

                    Ok(())
                }
                DataFormat::U8Iter(iter) => {
                    let mut writebuf = [0; 17];
                    let mut i = 1;
                    let len = writebuf.len();

                    // Data mode
                    writebuf[0] = self.data_byte;

                    for byte in iter.into_iter() {
                        writebuf[i] = byte;
                        i += 1;

                        if i == len {
                            self.i2c
                                .write(self.addr, &writebuf[0..=len])
                                .await
                                .map_err(|_| DisplayError::BusWriteError)?;
                            i = 1;
                        }
                    }

                    if i > 1 {
                        self.i2c
                            .write(self.addr, &writebuf[0..=i])
                            .await
                            .map_err(|_| DisplayError::BusWriteError)?;
                    }

                    Ok(())
                }
                _ => Err(DisplayError::DataFormatNotImplemented),
            }
        }
    }
}
