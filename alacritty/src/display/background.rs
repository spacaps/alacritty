use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Point};
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::vte::ansi::NamedColor;

use ascii_render::{AsciiOptions, AsciiRenderer, CellGlyph, GlyphFrameSeries, LayoutPolicy};

use image::codecs::gif::GifDecoder;
use image::{AnimationDecoder, DynamicImage};
use log::warn;

use crate::display::SizeInfo;
use crate::display::color::{List, Rgb};
use crate::display::content::RenderableCell;

const ADVANCE_INTERVAL: Duration = Duration::from_millis(120);

#[derive(Clone, Debug)]
struct BackgroundFrame {
    image: DynamicImage,
    delay: Duration,
}

struct SampleGifLoader;

impl SampleGifLoader {
    fn frames() -> Arc<Vec<BackgroundFrame>> {
        static CACHE: OnceLock<Arc<Vec<BackgroundFrame>>> = OnceLock::new();
        CACHE.get_or_init(|| Arc::new(Self::load_frames())).clone()
    }

    fn load_frames() -> Vec<BackgroundFrame> {
        let path = Self::sample_path();

        let file = match File::open(&path) {
            Ok(file) => file,
            Err(err) => {
                warn!("failed to open background GIF at {}: {err}", path.display());
                return Vec::new();
            },
        };

        let decoder = match GifDecoder::new(file) {
            Ok(decoder) => decoder,
            Err(err) => {
                warn!("failed to decode background GIF at {}: {err}", path.display());
                return Vec::new();
            },
        };

        match decoder.into_frames().collect_frames() {
            Ok(frames) => frames
                .into_iter()
                .map(|frame| {
                    let delay = Duration::from(frame.delay());
                    let buffer = frame.into_buffer();
                    let (w, h) = buffer.dimensions();
                    println!("frame dimensions: {}x{}", w, h);
                    let image = DynamicImage::ImageRgba8(buffer);
                    BackgroundFrame { image, delay }
                })
                .collect(),
            Err(err) => {
                warn!("failed to collect frames from background GIF at {}: {err}", path.display());
                Vec::new()
            },
        }
    }

    fn sample_path() -> PathBuf {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let project_root = manifest_dir.parent().unwrap_or(manifest_dir);
        project_root.join("sample.gif")
    }
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
}

impl BackgroundAnimation {
    pub fn new(size: &SizeInfo) -> Self {
        let source_frames = SampleGifLoader::frames();
        let (volume, frame_delays) = Self::create_volume(size, &source_frames);

        Self {
            volume,
            current_frame_index: 0,
            last_update: Instant::now(),
            needs_full_redraw: true,
            source_frames,
            frame_delays,
        }
    }

    pub fn on_resize(&mut self, size: &SizeInfo) {
        let (volume, frame_delays) = Self::create_volume(size, &self.source_frames);
        self.volume = volume;
        self.frame_delays = frame_delays;
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
    ) -> (GlyphFrameSeries, Vec<Duration>) {
        let empty_series = GlyphFrameSeries::new(0, 0, 0, Vec::new());
        if frames.is_empty() {
            return (empty_series, Vec::new());
        }

        let columns_limit = size.columns().min(u16::MAX as usize) as u16;
        let rows_limit = size.screen_lines().min(u16::MAX as usize) as u16;

        if columns_limit == 0 || rows_limit == 0 {
            return (empty_series, Vec::new());
        }
        let cell_width = size.cell_width();
        let cell_height = size.cell_height();
        let cell_aspect = if cell_width > f32::EPSILON { cell_width / cell_height } else { 1.0 };

        let layout =
            LayoutPolicy::FitViewport { columns: columns_limit, rows: rows_limit, cell_aspect };

        let renderer = AsciiRenderer::default();
        let options = AsciiOptions::default();

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
                            cells.reserve(frame_stride * frames.len());
                        }
                    }

                    delays.push(frame.delay);
                    cells.extend(grid.cells);
                },
                Err(err) => {
                    warn!("failed to render GIF frame to ASCII: {err}");
                },
            }
        }

        let Some((frame_width, frame_height)) = frame_dimensions else {
            return (empty_series, Vec::new());
        };

        let frame_count = delays.len();
        if frame_count == 0 {
            return (empty_series, Vec::new());
        }

        let frame_stride = usize::from(frame_width) * usize::from(frame_height);
        if frame_stride == 0 {
            return (empty_series, Vec::new());
        }

        let expected_len = frame_stride * frame_count;
        if cells.len() != expected_len {
            warn!(
                "dropping rendered frames due to unexpected cell count (expected {}, found {})",
                expected_len,
                cells.len()
            );
            return (empty_series, Vec::new());
        }

        (GlyphFrameSeries::new(frame_width, frame_height, frame_count, cells), delays)
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
