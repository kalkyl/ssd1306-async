# Async SSD1306 driver

Ported from [ssd1306](https://crates.io/crates/ssd1306) to async I2C/SPI using `embedded-hal-async`

[![CRIUS display showing the Rust logo](readme_banner.jpg?raw=true)](examples/image_i2c.rs)

Async I2C and SPI (4 wire) driver for the SSD1306 OLED display.

## [Documentation](https://docs.rs/ssd1306-async)

## [Changelog](CHANGELOG.md)

## [Examples](examples)

This crate uses [`probe-run`](https://crates.io/crates/probe-run) to run the examples. Once set up,
it should be as simple as `cargo run --example <example name> --release`.

From [`examples/image_i2c.rs`](examples/image_i2c.rs):



## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or
  http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the
work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any
additional terms or conditions.
