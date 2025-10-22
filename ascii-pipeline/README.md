# ASCII Pipeline Workspace

This workspace hosts a reusable library (`ascii_render`) and a CLI tool (`ascii_cli`) for converting raster images or animation frames into ASCII glyph grids. The crates are designed to be embedded into applications like Alacritty's background animation subsystem while remaining useful as standalone utilities.

## Prerequisites

- Rust toolchain (edition 2021) via [rustup](https://rustup.rs/)
- `cargo` for building and running the workspace

## Building

```bash
cargo build
```

Run the command from the workspace root (`ascii-pipeline/`). The build compiles both the library and the CLI crate.

## Library usage (`ascii_render`)

Add the crate to another project via a relative path dependency:

```bash
cargo add ascii_render --path /path/to/ascii-pipeline/crates/ascii_render
```

Example snippet:

```rust
use ascii_render::{AsciiRenderer, AsciiOptions, LayoutPolicy};

let renderer = AsciiRenderer::default();
let options = AsciiOptions::default();
let layout = LayoutPolicy::FixedColumns(120);
let output = renderer.render_path("horse.png", layout, options)?;

for row in output.grid.rows() {
    println!("{}", row);
}
```

## CLI usage (`ascii_cli`)

Preview ASCII output directly in the terminal:

```bash
cargo run -p ascii_cli -- preview horse.png --width 100
```

Export the ASCII art to a file:

```bash
cargo run -p ascii_cli -- convert horse.png --width 120 --output horse.txt
```

Generate frames from an animation while resampling to a terminal layout:

```bash
cargo run -p ascii_cli -- animate horse.gif --width 80 --fps 12 --out-dir frames/
```

Each CLI subcommand exposes `--help` for detailed flags and options.
