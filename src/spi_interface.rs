//! SPI interface factory

use crate::{DataFormat, DisplayError, WriteOnlyDataCommand};
use core::future::Future;
type Result = core::result::Result<(), DisplayError>;
use embedded_hal_async::spi::{SpiBus, SpiDevice};
use core::convert::Infallible;
use embedded_hal::digital::v2::OutputPin;

async fn send_u8<SPI>(spi: &mut SPI, words: DataFormat<'_>) -> Result
where
    SPI: SpiDevice,
    SPI::Bus: SpiBus,
{
    match words {
        DataFormat::U8(slice) => spi.write(slice).await.map_err(|_| DisplayError::BusWriteError),
        DataFormat::U16(slice) => {
            use byte_slice_cast::*;
            spi.write(slice.as_byte_slice())
                .await
                .map_err(|_| DisplayError::BusWriteError)
        }
        DataFormat::U16LE(slice) => {
            use byte_slice_cast::*;
            for v in slice.as_mut() {
                *v = v.to_le();
            }
            spi.write(slice.as_byte_slice())
                .await
                .map_err(|_| DisplayError::BusWriteError)
        }
        DataFormat::U16BE(slice) => {
            use byte_slice_cast::*;
            for v in slice.as_mut() {
                *v = v.to_be();
            }
            spi.write(slice.as_byte_slice())
                .await
                .map_err(|_| DisplayError::BusWriteError)
        }
        DataFormat::U8Iter(iter) => {
            let mut buf = [0; 32];
            let mut i = 0;

            for v in iter.into_iter() {
                buf[i] = v;
                i += 1;

                if i == buf.len() {
                    spi.write(&buf).await.map_err(|_| DisplayError::BusWriteError)?;
                    i = 0;
                }
            }

            if i > 0 {
                spi.write(&buf[..i])
                    .await
                    .map_err(|_| DisplayError::BusWriteError)?;
            }

            Ok(())
        }
        DataFormat::U16LEIter(iter) => {
            use byte_slice_cast::*;
            let mut buf = [0; 32];
            let mut i = 0;

            for v in iter.map(u16::to_le) {
                buf[i] = v;
                i += 1;

                if i == buf.len() {
                    spi.write(&buf.as_byte_slice())
                        .await
                        .map_err(|_| DisplayError::BusWriteError)?;
                    i = 0;
                }
            }

            if i > 0 {
                spi.write(&buf[..i].as_byte_slice())
                    .await
                    .map_err(|_| DisplayError::BusWriteError)?;
            }

            Ok(())
        }
        DataFormat::U16BEIter(iter) => {
            use byte_slice_cast::*;
            let mut buf = [0; 64];
            let mut i = 0;
            let len = buf.len();

            for v in iter.map(u16::to_be) {
                buf[i] = v;
                i += 1;

                if i == len {
                    spi.write(&buf.as_byte_slice())
                        .await
                        .map_err(|_| DisplayError::BusWriteError)?;
                    i = 0;
                }
            }

            if i > 0 {
                spi.write(&buf[..i].as_byte_slice())
                    .await
                    .map_err(|_| DisplayError::BusWriteError)?;
            }

            Ok(())
        }
        // _ => Err(DisplayError::DataFormatNotImplemented),
    }
}

/// SPI display interface.
///
/// This combines the SPI peripheral and a data/command as well as a chip-select pin
pub struct SPIInterface<SPI, DC, CS> {
    spi_no_cs: SPIInterfaceNoCS<SPI, DC>,
    cs: CS,
}

impl<SPI, DC, CS> SPIInterface<SPI, DC, CS>
where
    SPI: SpiDevice + 'static,
    SPI::Bus: SpiBus,
    DC: OutputPin<Error = Infallible> + 'static,
    CS: OutputPin<Error = Infallible> + 'static,
{
    /// Create new SPI interface for communication with a display driver
    pub fn new(spi: SPI, dc: DC, cs: CS) -> Self {
        Self {
            spi_no_cs: SPIInterfaceNoCS::new(spi, dc),
            cs,
        }
    }

    /// Consume the display interface and return
    /// the underlying peripherial driver and GPIO pins used by it
    pub fn release(self) -> (SPI, DC, CS) {
        let (spi, dc) = self.spi_no_cs.release();
        (spi, dc, self.cs)
    }

    async fn with_cs<'a, F, Fut>(
        &'a mut self,
        f: F
    ) 
         -> Result 
         
         where 
            F: FnOnce(&'a mut SPIInterfaceNoCS<SPI, DC>) -> Fut + 'a,
        Fut: Future<Output = Result> + 'a,{
        // Assert chip select pin
        self.cs.set_low().map_err(|_| DisplayError::CSError)?;

        let result = f(&mut self.spi_no_cs).await;

        // Deassert chip select pin
        self.cs.set_high().ok();

        result
    }
}

impl<SPI, DC, CS> WriteOnlyDataCommand for SPIInterface<SPI, DC, CS>
where
    SPI: SpiDevice + 'static,
    SPI::Bus: SpiBus,
    DC: OutputPin<Error = Infallible> + 'static,
    CS: OutputPin<Error = Infallible> + 'static,
{
    type Error = DisplayError;
    type WriteFuture<'a> = impl Future<Output = Result> + 'a where Self: 'a;
    type DataFuture<'a> = impl Future<Output = Result> + 'a where Self: 'a;

    fn send_commands<'a>(&'a mut self, cmds: DataFormat<'a>) -> Self::WriteFuture<'a> {
        async move {
            self.with_cs(|spi_no_cs| spi_no_cs.send_commands(cmds))
                .await
        }
    }

    fn send_data<'a>(&'a mut self, buf: DataFormat<'a>) -> Self::DataFuture<'a> {
        async move { self.with_cs(|spi_no_cs| spi_no_cs.send_data(buf)).await }
    }
}

/// SPI display interface.
///
/// This combines the SPI peripheral and a data/command pin
pub struct SPIInterfaceNoCS<SPI, DC> {
    spi: SPI,
    dc: DC,
}

impl<SPI, DC> SPIInterfaceNoCS<SPI, DC>
where
    SPI: SpiDevice,
    SPI::Bus: SpiBus,
    DC: OutputPin<Error = Infallible>,
{
    /// Create new SPI interface for communciation with a display driver
    pub fn new(spi: SPI, dc: DC) -> Self {
        Self { spi, dc }
    }

    /// Consume the display interface and return
    /// the underlying peripherial driver and GPIO pins used by it
    pub fn release(self) -> (SPI, DC) {
        (self.spi, self.dc)
    }
}

impl<SPI, DC> WriteOnlyDataCommand for SPIInterfaceNoCS<SPI, DC>
where
    SPI: SpiDevice,
    SPI::Bus: SpiBus,
    DC: OutputPin<Error = Infallible>,
{
    type Error = DisplayError;
    type WriteFuture<'a> = impl Future<Output = Result> + 'a where Self: 'a;
    type DataFuture<'a> = impl Future<Output = Result> + 'a where Self: 'a;

    fn send_commands<'a>(&'a mut self, cmds: DataFormat<'a>) -> Self::WriteFuture<'a> {
        async move {
            // 1 = data, 0 = command
            self.dc.set_low().map_err(|_| DisplayError::DCError)?;

            // Send words over SPI
            send_u8(&mut self.spi, cmds).await
        }
    }

    fn send_data<'a>(&'a mut self, buf: DataFormat<'a>) -> Self::DataFuture<'a> {
        async move {
            // 1 = data, 0 = command
            self.dc.set_high().map_err(|_| DisplayError::DCError)?;

            // Send words over SPI
            send_u8(&mut self.spi, buf).await
        }
    }
}
