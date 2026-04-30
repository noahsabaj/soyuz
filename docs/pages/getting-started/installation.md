# Installation

Get Soyuz running on your machine.

## Prerequisites

- Rust toolchain 1.85 or newer.
- Linux with X11 or Wayland for the current desktop workbench.

Linux is the primary target today. Docked native preview works best on X11; other sessions can use Pop Out preview.

## Build From Source

```text
git clone https://github.com/noahsabaj/soyuz
cd soyuz
cargo build --release -p soyuz-app --bin soyuz-studio
```

The release binary is written to `./target/release/soyuz-studio`.

## Run Soyuz Studio

```text
dx run --desktop --package soyuz-app --bin soyuz-studio --features desktop
```

`dx` is the Dioxus CLI. It bundles desktop assets and launches the Dioxus desktop app with the same asset pipeline used by normal development.
