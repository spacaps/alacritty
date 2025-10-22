mod ascii;
mod image_pipeline;

use std::path::Path;

use image::{DynamicImage, GenericImageView};

pub use ascii::{
    gradient::Gradient,
    grid::{CellGlyph, GlyphGrid},
    mapping::GlyphMapper,
};
pub use image_pipeline::{
    edges::{EdgeMode, EdgeSample},
    loader::FrameSource,
    resize::LayoutPolicy,
};

use image_pipeline::{adjust, edges, resize::TargetGeometry};

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
    /// Edge extraction mode.
    pub edge_mode: EdgeMode,
}

impl Default for AsciiOptions {
    fn default() -> Self {
        Self {
            gradient: Gradient::detailed(),
            invert: false,
            brightness: 0.0,
            contrast: 0.0,
            font_aspect: 0.55,
            edge_mode: EdgeMode::None,
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
        mut image: DynamicImage,
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

        let grid = match map {
            edges::EdgeResult::Intensity(mut intensities) => {
                mapper.map_intensity(&intensities, geometry.columns, geometry.rows)
            },
            edges::EdgeResult::Orientation(samples) => {
                mapper.map_orientation(&samples, geometry.columns, geometry.rows)
            },
        };

        Ok(RenderOutput { grid, geometry, assumed_font_aspect: options.font_aspect })
    }
}
