use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Point};
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::vte::ansi::NamedColor;

use ascii_render::{
    AsciiOptions, AsciiRenderer, CellGlyph, ColorMode, GlyphFrameSeries, LayoutPolicy,
};

use image::codecs::gif::GifDecoder;
use image::{AnimationDecoder, DynamicImage, GenericImageView};
use log::{debug, warn};

use crate::display::SizeInfo;
use crate::display::color::{List, Rgb};
use crate::display::content::RenderableCell;

const ADVANCE_INTERVAL: Duration = Duration::from_millis(120);

#[derive(Clone, Debug)]
pub struct BackgroundAnimationConfig {
    pub path: PathBuf,
    pub color_mode: ColorMode,
}

#[derive(Clone, Debug)]
struct BackgroundFrame {
    image: DynamicImage,
    delay: Duration,
}

/// State driving a simple background glyph animation.
#[derive(Debug)]
pub struct BackgroundAnimation {
    volume: GlyphFrameSeries,
    current_frame_index: usize,
    last_update: Instant,
    needs_full_redraw: bool,
    source_frames: Arc<Vec<BackgroundFrame>>,
    frame_delays: Vec<Duration>,
    color_mode: ColorMode,
}

impl BackgroundAnimation {
    pub fn new(size: &SizeInfo, config: BackgroundAnimationConfig) -> Option<Self> {
        let frames = match load_frames(&config.path) {
            Ok(frames) => frames,
            Err(err) => {
                warn!("failed to load background animation {}: {err}", config.path.display());
                return None;
            },
        };

        if frames.is_empty() {
            warn!("background animation {} contained no frames", config.path.display());
            return None;
        }

        let source_frames = Arc::new(frames);
        let (volume, frame_delays) =
            match Self::create_volume(size, &source_frames, config.color_mode) {
                Some(result) => result,
                None => {
                    warn!(
                        "background animation {} produced no renderable frames",
                        config.path.display()
                    );
                    return None;
                },
            };

        Some(Self {
            volume,
            current_frame_index: 0,
            last_update: Instant::now(),
            needs_full_redraw: true,
            source_frames,
            frame_delays,
            color_mode: config.color_mode,
        })
    }

    pub fn on_resize(&mut self, size: &SizeInfo) {
        if let Some((volume, frame_delays)) =
            Self::create_volume(size, &self.source_frames, self.color_mode)
        {
            self.volume = volume;
            self.frame_delays = frame_delays;
        } else {
            self.volume = GlyphFrameSeries::new(0, 0, 0, Vec::new());
            self.frame_delays.clear();
        }

        self.current_frame_index = 0;
        self.last_update = Instant::now();
        self.needs_full_redraw = true; // TODO: optimize redraws
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

        if now.duration_since(self.last_update) < self.current_frame_delay() {
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

    fn create_volume(
        size: &SizeInfo,
        frames: &[BackgroundFrame],
        color_mode: ColorMode,
    ) -> Option<(GlyphFrameSeries, Vec<Duration>)> {
        if frames.is_empty() {
            return None;
        }

        let columns_limit = size.columns().min(u16::MAX as usize) as u16;
        let rows_limit = size.screen_lines().min(u16::MAX as usize) as u16;

        if columns_limit == 0 || rows_limit == 0 {
            return None;
        }

        let cell_width = size.cell_width();
        let cell_height = size.cell_height();
        let cell_aspect = if cell_width > f32::EPSILON { cell_width / cell_height } else { 1.0 };

        let layout =
            LayoutPolicy::FitViewport { columns: columns_limit, rows: rows_limit, cell_aspect };

        let renderer = AsciiRenderer::default();
        let mut options = AsciiOptions::default();
        options.color_mode = color_mode; //TODO: use mode from config

        let mut frame_dimensions: Option<(u16, u16)> = None;
        let mut delays = Vec::with_capacity(frames.len());
        let mut cells: Vec<CellGlyph> = Vec::new();

        for frame in frames {
            match renderer.render_image(frame.image.clone(), layout, options.clone()) {
                Ok(output) => {
                    let grid = output.grid;
                    let width = grid.width;
                    let height = grid.height;

                    if width == 0 || height == 0 {
                        continue;
                    }

                    if let Some((expected_width, expected_height)) = frame_dimensions {
                        if expected_width != width || expected_height != height {
                            warn!(
                                "skipping background frame with mismatched dimensions {}x{}",
                                width, height
                            );
                            continue;
                        }
                    } else {
                        frame_dimensions = Some((width, height));
                        let frame_stride = usize::from(width) * usize::from(height);
                        if frame_stride > 0 {
                            cells.reserve(frame_stride.saturating_mul(frames.len()));
                        }
                    }

                    delays.push(frame.delay);
                    cells.extend(grid.cells);
                },
                Err(err) => warn!("failed to render GIF frame to ASCII: {err}"),
            }
        }

        let Some((frame_width, frame_height)) = frame_dimensions else {
            return None;
        };

        let frame_count = delays.len();
        if frame_count == 0 {
            return None;
        }

        let frame_stride = usize::from(frame_width) * usize::from(frame_height);
        if frame_stride == 0 {
            return None;
        }

        let expected_len = frame_stride * frame_count;
        if cells.len() != expected_len {
            warn!(
                "dropping rendered frames due to unexpected cell count (expected {}, found {})",
                expected_len,
                cells.len()
            );
            return None;
        }

        Some((GlyphFrameSeries::new(frame_width, frame_height, frame_count, cells), delays))
    }

    fn current_frame_delay(&self) -> Duration {
        if self.frame_delays.is_empty() {
            return ADVANCE_INTERVAL;
        }

        let index = self.current_frame_index.min(self.frame_delays.len() - 1);
        let delay = self.frame_delays[index];
        if delay.is_zero() { ADVANCE_INTERVAL } else { delay }
    }
}

fn load_frames(path: &Path) -> Result<Vec<BackgroundFrame>, String> {
    match path.extension().and_then(|ext| ext.to_str()).map(|ext| ext.to_ascii_lowercase()) {
        Some(ext) if ext == "gif" => load_frames_from_gif(path),
        _ => load_frames_from_image(path),
    }
    
}

fn load_frames_from_gif(path: &Path) -> Result<Vec<BackgroundFrame>, String> {
    let file =
        File::open(path).map_err(|err| format!("failed to open gif {}: {err}", path.display()))?;
    let decoder = GifDecoder::new(file)
        .map_err(|err| format!("failed to decode gif {}: {err}", path.display()))?;
    let frames = decoder
        .into_frames()
        .collect_frames()
        .map_err(|err| format!("failed to collect frames from {}: {err}", path.display()))?;

    let mut result = Vec::with_capacity(frames.len());
    for frame in frames {
        let delay = Duration::from(frame.delay());
        let buffer = frame.into_buffer();
        let (w, h) = buffer.dimensions();
        debug!("loaded background frame {}x{} from {}", w, h, path.display());
        let image = DynamicImage::ImageRgba8(buffer);
        result.push(BackgroundFrame { image, delay });
    }

    Ok(result)
}

fn load_frames_from_image(path: &Path) -> Result<Vec<BackgroundFrame>, String> {
    let image = image::open(path)
        .map_err(|err| format!("failed to open image {}: {err}", path.display()))?;
    let (w, h) = image.dimensions();
    debug!("loaded background image {}x{} from {}", w, h, path.display());
    Ok(vec![BackgroundFrame { image, delay: ADVANCE_INTERVAL }])
}
