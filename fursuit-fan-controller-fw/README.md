# `fursuit-fan-controller-fw`

This is the firmware to run on the STM32F103C8T6 of this project.

The microcontroller generates a speed control PWM signal for the attached fan. The speed can be selected with the plus and minus buttons. The current level is indicated by an RGB LED.

## Hardware setup

1. Attach an ST-Link programmer to your computer via USB. (Have the necessary drivers installed.)
2. Wire up SWDIO, SWCLK, and GND from the programmer to the board.
3. Power the board via USB Type C.

## Software setup

1. Install Rust (https://rust-lang.org/)
1. `rustup toolchain install thumbv7m-none-eabi`
2. `cargo install flip-link`
3. Install probe-rs by following the instructions at https://probe.rs/docs/getting-started/installation/
4. You might need to install drivers for your ST-Link.

After this, you should be able to see your ST-Link by running:

```bash
probe-rs list
```

The output could look something like this:

```
The following debug probes were found:
[0]: STLink V2 -- 0483:3748: (ST-LINK)
```

## Development workflows

To compile, flash the board and then observe log output:

```bash
# Development
cargo run
# Optimized build
cargo run --production
```


## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or
  http://www.apache.org/licenses/LICENSE-2.0)

- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
licensed as above, without any additional terms or conditions.
