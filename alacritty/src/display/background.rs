use std::time::{Duration, Instant};

use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Point};
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::vte::ansi::NamedColor;

use ascii_render::{CellGlyph, GlyphGrid};

use crate::display::SizeInfo;
use crate::display::color::{List, Rgb};
use crate::display::content::RenderableCell;

const ADVANCE_INTERVAL: Duration = Duration::from_millis(120);

/// State driving a simple background glyph animation.
#[derive(Debug)]
pub struct BackgroundAnimation {
    grid: GlyphGrid,
    last_update: Instant,
    needs_full_redraw: bool,
}

impl BackgroundAnimation {
    pub fn new(size: &SizeInfo) -> Self {
        Self { grid: Self::create_grid(size), last_update: Instant::now(), needs_full_redraw: true }
    }

    pub fn on_resize(&mut self, size: &SizeInfo) {
        self.grid = Self::create_grid(size);
        self.last_update = Instant::now();
        self.needs_full_redraw = true;
    }

    pub fn update(&mut self, now: Instant, size: &SizeInfo) -> bool {
        if !self.is_active(size) {
            return false;
        }

        if self.needs_full_redraw {
            self.needs_full_redraw = false;
            self.last_update = now;
            return true;
        }

        if now.duration_since(self.last_update) < ADVANCE_INTERVAL {
            return false;
        }

        self.last_update = now;
        self.advance_frame();
        true
    }

    pub fn render_cells(&self, colors: &List, size: &SizeInfo, out: &mut Vec<RenderableCell>) {
        if !self.is_active(size) {
            return;
        }

        let width = self.grid.width as usize;
        if width == 0 {
            return;
        }

        let visible_columns = size.columns().min(width);
        let visible_lines = size.screen_lines().min(self.grid.height as usize);

        if visible_columns == 0 || visible_lines == 0 {
            return;
        }

        let default_bg = colors[NamedColor::Background];

        out.reserve(visible_columns * visible_lines);

        for line in 0..visible_lines {
            for column in 0..visible_columns {
                let idx = line * width + column;
                let cell = &self.grid.cells[idx];

                let fg = Rgb::new(cell.fg[0], cell.fg[1], cell.fg[2]);
                let (bg, bg_alpha) = match cell.bg {
                    Some(color) => (Rgb::new(color[0], color[1], color[2]), 1.0),
                    None => (default_bg, 0.0),
                };

                out.push(RenderableCell {
                    character: cell.ch,
                    point: Point::new(line, Column(column)),
                    fg,
                    bg,
                    bg_alpha,
                    underline: fg,
                    flags: Flags::DIM,
                    extra: None,
                });
            }
        }
    }

    pub fn is_active(&self, size: &SizeInfo) -> bool {
        size.columns() > 0 && size.screen_lines() > 0 && self.grid.width > 0 && self.grid.height > 0
    }

    fn advance_frame(&mut self) {
        let width = self.grid.width as usize;
        let height = self.grid.height as usize;

        if width == 0 || height == 0 {
            return;
        }

        for row in 0..height {
            let start = row * width;
            let end = start + width;
            self.grid.cells[start..end].rotate_left(1);
        }
    }

    fn create_grid(size: &SizeInfo) -> GlyphGrid {
        let columns = size.columns().min(u16::MAX as usize) as u16;
        let lines = size.screen_lines().min(u16::MAX as usize) as u16;

        let total_cells = usize::from(columns) * usize::from(lines);
        if total_cells == 0 {
            return GlyphGrid::new(columns, lines, Vec::new());
        }

        let palette = ['.', ':', '-', '=', '+', '*', '#', '%', '@'];
        let mut cells = Vec::with_capacity(total_cells);

        let width = columns as usize;
        for line in 0..lines as usize {
            for column in 0..width {
                let index = (line + column) % palette.len();
                let ch = palette[index];
                let denominator = palette.len().saturating_sub(1).max(1) as f32;
                let intensity = (index as f32) / denominator;
                let value = (intensity.clamp(0.0, 1.0) * 255.0).round() as u8;

                cells.push(CellGlyph { ch, fg: [value, value, value], bg: None, alpha: 1.0 });
            }
        }

        GlyphGrid::new(columns, lines, cells)
    }
}
