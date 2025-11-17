mod ascii;
mod image_pipeline;

use std::path::Path;

use image::{DynamicImage, GenericImageView};

const TRANSPARENT_ALPHA_THRESHOLD: f32 = 0.001;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ColorMode {
    Luminance,
    ColorAlpha,
}

pub use ascii::{
    gradient::Gradient,
    grid::{CellGlyph, GlyphFrameSeries, GlyphGrid},
    mapping::GlyphMapper,
    series::{GlyphGridFrame, GlyphGridSeries},
};
pub use image_pipeline::{
    edges::{EdgeMode, EdgeSample},
    resize::{LayoutPolicy, TargetGeometry},
};

use image_pipeline::{adjust, edges};

#[derive(Debug, thiserror::Error)]
pub enum AsciiError {
    #[error("failed to load image: {0}")]
    Image(#[from] image::ImageError),
    #[error("unsupported layout dimensions")]
    InvalidLayout,
}

#[derive(Clone, Debug)]
pub struct AsciiOptions {
    pub gradient: Gradient,
    pub invert: bool,
    /// Brightness offset in the range [-255.0, 255.0].
    pub brightness: f32,
    /// Contrast offset in the range [-255.0, 255.0].
    pub contrast: f32,
    /// Font aspect ratio (height / width) assumed when deriving grid size.
    pub font_aspect: f32,
    /// Sobel edge detection mode, or None
    pub edge_mode: EdgeMode,
    /// color: char gradient is alpha, luminance: char gradient is luminance
    pub color_mode: ColorMode,
}

impl Default for AsciiOptions {
    fn default() -> Self {
        Self {
            gradient: Gradient::blocks(),
            invert: false,
            brightness: 0.0,
            contrast: 0.0,
            font_aspect: 0.55,
            edge_mode: EdgeMode::None,
            color_mode: ColorMode::Luminance,
        }
    }
}

#[derive(Clone, Debug)]
pub struct RenderOutput {
    pub grid: GlyphGrid,
    pub geometry: TargetGeometry,
    /// Font aspect ratio used to derive the layout.
    pub assumed_font_aspect: f32,
}

#[derive(Default)]
pub struct AsciiRenderer;

impl AsciiRenderer {
    pub fn render_path<P: AsRef<Path>>(
        &self,
        path: P,
        layout: LayoutPolicy,
        options: AsciiOptions,
    ) -> Result<RenderOutput, AsciiError> {
        let image = image::open(path)?;
        self.render_image(image, layout, options)
    }

    pub fn render_image(
        &self,
        image: DynamicImage,
        layout: LayoutPolicy,
        options: AsciiOptions,
    ) -> Result<RenderOutput, AsciiError> {
        let (width, height) = image.dimensions();
        let geometry =
            layout.derive(width, height, options.font_aspect).ok_or(AsciiError::InvalidLayout)?;

        let resized = image.resize_exact(
            geometry.columns as u32,
            geometry.rows as u32,
            image::imageops::FilterType::CatmullRom,
        );

        let rgba = resized.to_rgba8();
        let pixel_data = rgba.into_raw();

        let mut luminance = adjust::extract_luma(&resized, options.invert);
        adjust::apply_contrast_and_brightness(&mut luminance, options.contrast, options.brightness);

        let map = match options.edge_mode {
            EdgeMode::None => edges::EdgeResult::Intensity(luminance),
            EdgeMode::Sobel { threshold } => {
                let intensities =
                    edges::sobel_map(&luminance, geometry.columns, geometry.rows, threshold);
                edges::EdgeResult::Intensity(intensities)
            },
        };

        let mut mapper = GlyphMapper::new(options.gradient.clone());

        let mut grid = match map {
            edges::EdgeResult::Intensity(intensities) => {
                mapper.map_intensity(&intensities, geometry.columns, geometry.rows)
            },
            edges::EdgeResult::Orientation(samples) => {
                mapper.map_orientation(&samples, geometry.columns, geometry.rows)
            },
        };

        let pixel_count = pixel_data.len() / 4;
        if pixel_count == grid.cells.len() {
            for (idx, cell) in grid.cells.iter_mut().enumerate() {
                let start = idx * 4;
                let r = pixel_data[start];
                let g = pixel_data[start + 1];
                let b = pixel_data[start + 2];
                let alpha = (pixel_data[start + 3] as f32 / 255.0).clamp(0.0, 1.0);

                cell.fg = [r, g, b];
                cell.alpha = alpha;
                if alpha <= TRANSPARENT_ALPHA_THRESHOLD {
                    cell.ch = ' ';
                }
            }
        }

        Ok(RenderOutput { grid, geometry, assumed_font_aspect: options.font_aspect })
    }
}
