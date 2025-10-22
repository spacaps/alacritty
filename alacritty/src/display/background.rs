use std::time::{Duration, Instant};

use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Point};

use crate::display::SizeInfo;
use crate::display::color::List;
use crate::display::content::RenderableCell;
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::vte::ansi::NamedColor;

const ADVANCE_INTERVAL: Duration = Duration::from_millis(120);

/// State driving a simple background glyph animation.
#[derive(Debug)]
pub struct BackgroundAnimation {
    glyph: char,
    column: Option<usize>,
    row: usize,
    last_update: Instant,
}

impl BackgroundAnimation {
    pub fn new(size: &SizeInfo) -> Self {
        let mut animation = Self { glyph: 'a', column: None, row: 0, last_update: Instant::now() };
        animation.on_resize(size);
        animation
    }

    pub fn on_resize(&mut self, size: &SizeInfo) {
        if size.screen_lines() == 0 || size.columns() == 0 {
            self.column = None;
            self.row = 0;
            return;
        }

        self.row = size.screen_lines().saturating_sub(1) / 2;
        let max_column = size.columns().saturating_sub(1);
        self.column = Some(self.column.unwrap_or(max_column).min(max_column));
    }

    pub fn update(
        &mut self,
        now: Instant,
        size: &SizeInfo,
    ) -> Option<(Option<Point<usize>>, Point<usize>)> {
        if size.screen_lines() == 0 || size.columns() == 0 {
            self.column = None;
            return None;
        }

        if self.column.is_none() {
            self.column = Some(size.columns().saturating_sub(1));
            self.last_update = now;
            return self.current_point(size).map(|point| (None, point));
        }

        if now.duration_since(self.last_update) < ADVANCE_INTERVAL {
            return None;
        }

        self.last_update = now;

        let old_point = self.current_point(size);
        let mut column = self.column.unwrap_or(0);
        if column == 0 {
            column = size.columns().saturating_sub(1);
        } else {
            column -= 1;
        }
        self.column = Some(column);

        self.current_point(size).map(|new_point| (old_point, new_point))
    }

    pub fn render_cell(&self, colors: &List, size: &SizeInfo) -> Option<RenderableCell> {
        let point = self.current_point(size)?;
        let fg = colors[NamedColor::DimForeground];
        let bg = colors[NamedColor::Background];

        Some(RenderableCell {
            character: self.glyph,
            point,
            fg,
            bg,
            bg_alpha: 0.0,
            underline: fg,
            flags: Flags::DIM,
            extra: None,
        })
    }

    pub fn is_active(&self, size: &SizeInfo) -> bool {
        size.columns() > 0 && size.screen_lines() > 0 && self.column.is_some()
    }

    fn current_point(&self, size: &SizeInfo) -> Option<Point<usize>> {
        let column = self.column?;
        if size.columns() == 0 || size.screen_lines() == 0 {
            return None;
        }

        if column >= size.columns() {
            return None;
        }

        let line = self.row.min(size.screen_lines().saturating_sub(1));
        Some(Point::new(line, Column(column)))
    }
}
