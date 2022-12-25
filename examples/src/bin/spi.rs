#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]

use defmt::*;
use embassy_executor::Spawner;
use embassy_rp::gpio::{Level, Output};
use embassy_rp::spi;
use embassy_rp::spi::{Config, Spi};
use embassy_time::{Duration, Timer};
use embedded_graphics::{
    image::Image,
    mono_font::{ascii::FONT_9X18_BOLD, MonoTextStyleBuilder},
    pixelcolor::{BinaryColor, Rgb565},
    prelude::*,
    text::{Baseline, Text},
};
use embedded_hal_async::spi::ExclusiveDevice;
use ssd1306_async::{prelude::*, Ssd1306};
use tinybmp::Bmp;
use {defmt_rtt as _, panic_probe as _};

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_rp::init(Default::default());
    info!("Hello World!");

    // this is labelled RES on SSD1306. not present in all configurations.
    let rst = p.PIN_15;
    // this is labelled CS on SSD1306
    let cs = p.PIN_9;
    // this is labelled DC or D/C on SSD1306. not present in all configurations.
    let dc = p.PIN_8;
    // this is labelled D1 or SDIN on SSD1306
    let mosi = p.PIN_11;
    // this is labelled D0 or SCLK on SSD1306
    let clk = p.PIN_10;

    // create SPI
    let mut config = spi::Config::default();
    // Start with low SPI frequency until we know it's working
    config.frequency = 200_000;
    config.phase = spi::Phase::CaptureOnSecondTransition;
    config.polarity = spi::Polarity::IdleHigh;

    let spi = Spi::new_txonly(p.SPI1, clk, mosi, p.DMA_CH0, Config::default());

    let dc = Output::new(dc, Level::Low);
    let cs = Output::new(cs, Level::Low);

    // Start with the display in reset for 100ms
    // If your screen doesn't have reset you can skip this step
    let mut rst = Output::new(rst, Level::Low);
    Timer::after(Duration::from_millis(100)).await;
    rst.set_high();

    // We aren't sharing this SPI bus so use ExclusiveDevice.
    let device = ExclusiveDevice::new(spi, cs);
    let interface = ssd1306_async::SPIInterface::new(device, dc);

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
