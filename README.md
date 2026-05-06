# Embedded-Rust.com sample codes

This repository is an archived repository for [embedded-rust.com/](http://embedded-rust.com/).

## How to use this repository

### Setup the environment

You can install the rustc and cargo at [rustup.rs](https://rustup.rs/).

### clone

```bash
git clone https://github.com/ryota42/embedded-rust.git
```

### Embedded Rust environments

You need to specify the target MCU/MPU board, and target Rust toolchain.
For example, if you use the STM32F407Discovery board, you need to install the `thumbv7em-none-eabi` target toolchain. The required target toolchain is described in the rustc repository.

reference:
[rustc platform-support](https://doc.rust-lang.org/nightly/rustc/platform-support/arm-none-eabi.html)
[rustc thumbv7em info](https://doc.rust-lang.org/beta/rustc/platform-support/thumbv7em-none-eabi.html)

```bash
# Install the target Rust toolchain
cargo install thumbv7em-none-eabi
```

