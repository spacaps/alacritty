use std::time::{Duration, Instant};

use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Point};
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::vte::ansi::NamedColor;

use ascii_render::{CellGlyph, GlyphFrameSeries, LayoutPolicy};
use rand::Rng;

use crate::display::SizeInfo;
use crate::display::color::{List, Rgb};
use crate::display::content::RenderableCell;

const ADVANCE_INTERVAL: Duration = Duration::from_millis(120);

/// State driving a simple background glyph animation.
#[derive(Debug)]
pub struct BackgroundAnimation {
    volume: GlyphFrameSeries,
    current_frame_index: usize,
    last_update: Instant,
    needs_full_redraw: bool,
}

impl BackgroundAnimation {
    pub fn new(size: &SizeInfo) -> Self {
        Self {
            volume: Self::create_volume(size),
            current_frame_index: 0,
            last_update: Instant::now(),
            needs_full_redraw: true,
        }
    }

    pub fn on_resize(&mut self, size: &SizeInfo) {
        self.volume = Self::create_volume(size);
        self.current_frame_index = 0;
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

        let width = self.volume.width as usize;
        if width == 0 {
            return;
        }

        let visible_columns = size.columns().min(width);
        let visible_lines = size.screen_lines().min(self.volume.height as usize);

        if visible_columns == 0 || visible_lines == 0 {
            return;
        }

        let frame_cells = self.current_frame_cells();
        if frame_cells.is_empty() {
            return;
        }

        let default_bg = colors[NamedColor::Background];

        out.reserve(visible_columns * visible_lines);

        for line in 0..visible_lines {
            for column in 0..visible_columns {
                let idx = line * width + column;
                let cell = &frame_cells[idx];

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
        size.columns() > 0
            && size.screen_lines() > 0
            && self.volume.width > 0
            && self.volume.height > 0
            && self.volume.frame_count() > 0
            && !self.volume.is_empty()
    }

    fn advance_frame(&mut self) {
        if self.volume.frame_count() <= 1 {
            return;
        }

        self.current_frame_index = (self.current_frame_index + 1) % self.volume.frame_count();
    }

    pub fn current_frame_cells(&self) -> &[CellGlyph] {
        self.volume.frame(self.current_frame_index).unwrap_or(&[])
    }

    fn create_volume(size: &SizeInfo) -> GlyphFrameSeries {
        let columns_limit = size.columns().min(u16::MAX as usize) as u16;
        let rows_limit = size.screen_lines().min(u16::MAX as usize) as u16;

        if columns_limit == 0 || rows_limit == 0 {
            return GlyphFrameSeries::new(columns_limit, rows_limit, 0, Vec::new());
        }

        let cell_width = size.cell_width();
        let cell_height = size.cell_height();
        let cell_aspect = if cell_width > f32::EPSILON { cell_height / cell_width } else { 1.0 };

        let layout =
            LayoutPolicy::FitViewport { columns: columns_limit, rows: rows_limit, cell_aspect };

        let source_width = u32::from(columns_limit);
        let source_height = ((rows_limit as f32) / cell_aspect).round().max(1.0) as u32;

        let (columns, rows) = layout
            .derive(source_width, source_height, cell_aspect)
            .map(|geometry| (geometry.columns, geometry.rows))
            .unwrap_or((columns_limit, rows_limit));

        let frame_width = columns;
        let frame_height = rows;
        let frame_stride = usize::from(frame_width) * usize::from(frame_height);

        if frame_stride == 0 {
            return GlyphFrameSeries::new(frame_width, frame_height, 0, Vec::new());
        }

        let frame_count = 16usize;
        let palette = ['.', ':', '-', '=', '+', '*', '#', '%', '@'];
        let mut rng = rand::thread_rng();
        let mut cells = Vec::with_capacity(frame_stride * frame_count);

        let denominator = (palette.len().saturating_sub(1).max(1)) as f32;

        for _ in 0..frame_count {
            for _ in 0..frame_stride {
                let index = rng.gen_range(0..palette.len());
                let ch = palette[index];
                let intensity = (index as f32) / denominator;
                let value = (intensity.clamp(0.0, 1.0) * 255.0).round() as u8;

                cells.push(CellGlyph { ch, fg: [value, value, value], bg: None, alpha: 1.0 });
            }
        }

        GlyphFrameSeries::new(frame_width, frame_height, frame_count, cells)
    }
}
