#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]

use defmt::*;
use embassy_embedded_hal::shared_bus::asynch::i2c::I2cDevice;
use embassy_executor::Spawner;
use embassy_rp::bind_interrupts;
use embassy_rp::i2c::{Config, I2c, InterruptHandler};
use embassy_rp::peripherals::I2C0;
use embassy_sync::blocking_mutex::raw::ThreadModeRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_time::{Duration, Timer};
use embedded_graphics::{
    image::Image,
    mono_font::{ascii::FONT_9X18_BOLD, MonoTextStyleBuilder},
    pixelcolor::{BinaryColor, Rgb565},
    prelude::*,
    text::{Baseline, Text},
};
use ssd1306_async::{prelude::*, I2CDisplayInterface, Ssd1306};
use static_cell::make_static;
use tinybmp::Bmp;
use {defmt_rtt as _, panic_probe as _};

bind_interrupts!(struct Irqs {
    I2C0_IRQ => InterruptHandler<I2C0>;
});

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_rp::init(Default::default());
    info!("Hello World!");

    let scl = p.PIN_9;
    let sda = p.PIN_8;
    let mut config = Config::default();
    config.frequency = 400_000;
    let i2c = I2c::new_async(p.I2C0, scl, sda, Irqs, config);

    let i2c_bus: &'static _ = make_static!(Mutex::<ThreadModeRawMutex, _>::new(i2c));

    let interface = I2CDisplayInterface::new(I2cDevice::new(i2c_bus));
    let mut display = Ssd1306::new(interface, DisplaySize128x64, DisplayRotation::Rotate0)
        .into_buffered_graphics_mode();
    display.init().await.unwrap();

    let bmp = Bmp::from_slice(include_bytes!("../../rust.bmp")).expect("Failed to load BMP image");

    // The image is an RGB565 encoded BMP, so specifying the type as `Image<Bmp<Rgb565>>` will read
    // the pixels correctly
    let im: Image<Bmp<Rgb565>> = Image::new(&bmp, Point::new(32, 0));

    // We use the `color_converted` method here to automatically convert the RGB565 image data into
    // BinaryColor values.
    im.draw(&mut display.color_converted()).unwrap();
    display.flush().await.unwrap();

    loop {
        Timer::after(Duration::from_millis(1_000)).await;
        info!("Tick");
        display.clear();
        let text_style = MonoTextStyleBuilder::new()
            .font(&FONT_9X18_BOLD)
            .text_color(BinaryColor::On)
            .build();
        Text::with_baseline("Hello world!", Point::zero(), text_style, Baseline::Top)
            .draw(&mut display)
            .unwrap();
        Text::with_baseline("Hello Rust!", Point::new(0, 16), text_style, Baseline::Top)
            .draw(&mut display)
            .unwrap();

        display.flush().await.unwrap();

        Timer::after(Duration::from_millis(1_000)).await;
        info!("Tick");
        display.clear();
        im.draw(&mut display.color_converted()).unwrap();
        display.flush().await.unwrap();
    }
}
