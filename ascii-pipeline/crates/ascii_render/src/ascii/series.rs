use std::time::Duration;

use super::grid::GlyphGrid;
use crate::image_pipeline::resize::{LayoutPolicy, TargetGeometry};

#[derive(Clone, Debug)]
pub struct GlyphGridFrame {
    pub grid: GlyphGrid,
    pub duration: Duration,
}

#[derive(Clone, Debug, Default)]
pub struct GlyphGridSeries {
    frames: Vec<GlyphGridFrame>,
    total_duration: Duration,
    geometry: Option<TargetGeometry>,
}

impl GlyphGridSeries {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }

    pub fn len(&self) -> usize {
        self.frames.len()
    }

    pub fn geometry(&self) -> Option<TargetGeometry> {
        self.geometry
    }

    pub fn set_geometry(&mut self, geometry: TargetGeometry) {
        self.geometry = Some(geometry);
    }

    pub fn clear(&mut self) {
        self.frames.clear();
        self.total_duration = Duration::ZERO;
    }

    pub fn push_frame(&mut self, frame: GlyphGridFrame) {
        if let Some(geometry) = self.geometry {
            debug_assert_eq!(geometry.columns, frame.grid.width);
            debug_assert_eq!(geometry.rows, frame.grid.height);
        } else {
            let columns = frame.grid.width;
            let rows = frame.grid.height;
            let cell_aspect = if columns == 0 { 1.0 } else { rows as f32 / columns as f32 };
            self.geometry = Some(TargetGeometry { columns, rows, cell_aspect });
        }

        self.total_duration += frame.duration;
        self.frames.push(frame);
    }

    pub fn rebuild_from<F>(&mut self, frame_count: usize, frame_duration: Duration, mut builder: F)
    where
        F: FnMut(usize, TargetGeometry) -> GlyphGrid,
    {
        self.clear();

        if frame_count == 0 {
            return;
        }

        let Some(geometry) = self.geometry else {
            return;
        };

        if geometry.columns == 0 || geometry.rows == 0 {
            return;
        }

        for index in 0..frame_count {
            let grid = builder(index, geometry);
            let frame = GlyphGridFrame { grid, duration: frame_duration };
            self.total_duration += frame_duration;
            self.frames.push(frame);
        }

        if frame_duration.is_zero() {
            self.total_duration = Duration::ZERO;
        }
    }

    pub fn total_duration(&self) -> Duration {
        if self.frames.is_empty() {
            Duration::ZERO
        } else {
            self.total_duration
        }
    }

    pub fn normalize_elapsed(&self, elapsed: Duration) -> Duration {
        if self.frames.len() <= 1 {
            return Duration::ZERO;
        }

        let total = self.total_duration();
        if total.is_zero() {
            return Duration::ZERO;
        }

        let total_nanos = total.as_nanos();
        let remainder = elapsed.as_nanos() % total_nanos;
        let secs = (remainder / 1_000_000_000) as u64;
        let nanos = (remainder % 1_000_000_000) as u32;

        Duration::new(secs, nanos)
    }

    pub fn frame_index_at(&self, elapsed: Duration) -> Option<usize> {
        if self.frames.is_empty() {
            return None;
        }

        if self.frames.len() == 1 {
            return Some(0);
        }

        let mut remaining = self.normalize_elapsed(elapsed);

        for (index, frame) in self.frames.iter().enumerate() {
            if frame.duration.is_zero() || remaining < frame.duration {
                return Some(index);
            }
            remaining -= frame.duration;
        }

        Some(self.frames.len() - 1)
    }

    pub fn frame_at(&self, elapsed: Duration) -> Option<&GlyphGrid> {
        let index = self.frame_index_at(elapsed)?;
        self.frames.get(index).map(|frame| &frame.grid)
    }

    pub fn frame(&self, index: usize) -> Option<&GlyphGrid> {
        self.frames.get(index).map(|frame| &frame.grid)
    }

    pub fn update_geometry_from_layout(
        &mut self,
        layout: LayoutPolicy,
        source_width: u32,
        source_height: u32,
        default_aspect: f32,
    ) -> Option<TargetGeometry> {
        let geometry = layout.derive(source_width, source_height, default_aspect)?;
        let changed = match self.geometry {
            Some(current) => current != geometry,
            None => true,
        };

        if changed {
            self.geometry = Some(geometry);
        }

        self.geometry
    }
}
