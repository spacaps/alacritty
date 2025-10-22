use crate::image_pipeline::edges::EdgeSample;

use super::{
    gradient::Gradient,
    grid::{CellGlyph, GlyphGrid},
};

pub struct GlyphMapper {
    gradient: Gradient,
}

impl GlyphMapper {
    pub fn new(gradient: Gradient) -> Self {
        Self { gradient }
    }

    pub fn map_intensity(&mut self, intensities: &[f32], width: u16, height: u16) -> GlyphGrid {
        let mut cells = Vec::with_capacity(intensities.len());
        let gradient_chars = self.gradient.chars();
        let max_index = gradient_chars.len() - 1;

        for &value in intensities {
            let normalized = value.clamp(0.0, 1.0);
            let index = (normalized * max_index as f32).round() as usize;
            let ch = gradient_chars[index.min(max_index)];
            cells.push(CellGlyph::new(ch, normalized));
        }

        GlyphGrid::new(width, height, cells)
    }

    pub fn map_orientation(
        &mut self,
        samples: &[EdgeSample],
        width: u16,
        height: u16,
    ) -> GlyphGrid {
        let mut cells = Vec::with_capacity(samples.len());
        for sample in samples {
            if !sample.active {
                cells.push(CellGlyph::new(' ', 0.0));
                continue;
            }

            let ch = orientation_glyph(sample.angle_degrees);
            cells.push(CellGlyph::new(ch, sample.magnitude));
        }

        GlyphGrid::new(width, height, cells)
    }
}

fn orientation_glyph(angle: f32) -> char {
    let angle = angle.rem_euclid(180.0);
    if (0.0..22.5).contains(&angle) || (157.5..180.0).contains(&angle) {
        '-'
    } else if (22.5..67.5).contains(&angle) {
        '/'
    } else if (67.5..112.5).contains(&angle) {
        '|'
    } else {
        '\\'
    }
}
