egui-winit 0.31.1 (MIT OR Apache-2.0, https://github.com/emilk/egui) with two Dragonize changes in
src/lib.rs, both for recording macros: `Modifiers::mac_cmd` follows the Super / Windows key on every
platform (upstream: macOS only), and Ctrl+C / X / V are still sent as key events next to Copy / Cut /
Paste (upstream drops them). Wired in through `[patch.crates-io]` in ../../Cargo.toml.
