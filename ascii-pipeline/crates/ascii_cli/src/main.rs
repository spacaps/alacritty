use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use ascii_render::{AsciiOptions, AsciiRenderer, EdgeMode, Gradient, LayoutPolicy};
use clap::{Parser, Subcommand, ValueEnum};
use image::codecs::gif::GifDecoder;
use image::{AnimationDecoder, DynamicImage, Frame};
use indicatif::{ProgressBar, ProgressStyle};
use walkdir::WalkDir;

#[derive(Parser, Debug)]
#[command(author, version, about = "Convert images or animations to ASCII glyph grids")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Render ASCII art to stdout for a quick preview
    Preview(PreviewArgs),
    /// Convert an image to ASCII and write the result to disk
    Convert(ConvertArgs),
    /// Convert an animation (GIF or directory of frames) to ASCII frame files
    Animate(AnimateArgs),
}

#[derive(Parser, Debug)]
struct PreviewArgs {
    /// Input image path
    input: PathBuf,
    /// Target column width
    #[arg(long, default_value_t = 100)]
    width: u16,
    #[command(flatten)]
    settings: RenderSettings,
}

#[derive(Parser, Debug)]
struct ConvertArgs {
    /// Input image path
    input: PathBuf,
    /// Output file path
    #[arg(short, long)]
    output: PathBuf,
    /// Target column width
    #[arg(long, default_value_t = 120)]
    width: u16,
    #[command(flatten)]
    settings: RenderSettings,
}

#[derive(Parser, Debug)]
struct AnimateArgs {
    /// Input animation path (GIF file or directory of images)
    input: PathBuf,
    /// Output directory for frame files
    #[arg(short, long)]
    out_dir: PathBuf,
    /// Target column width
    #[arg(long, default_value_t = 120)]
    width: u16,
    /// Override frames per second when the input lacks timing information
    #[arg(long, default_value_t = 12.0)]
    fps: f32,
    #[command(flatten)]
    settings: RenderSettings,
}

#[derive(Parser, Debug, Clone)]
struct RenderSettings {
    /// Gradient preset used to map intensity to glyphs
    #[arg(long, value_enum, default_value = "detailed")]
    gradient: GradientPreset,
    /// Brightness adjustment (-255..255)
    #[arg(long, default_value_t = 0.0)]
    brightness: f32,
    /// Contrast adjustment (-255..255)
    #[arg(long, default_value_t = 0.0)]
    contrast: f32,
    /// Invert luminance before processing
    #[arg(long, default_value_t = false)]
    invert: bool,
    /// Font aspect ratio (height / width)
    #[arg(long, default_value_t = 0.55)]
    font_aspect: f32,
    /// Edge detection strategy
    #[arg(long, value_enum, default_value = "none")]
    edge: EdgeChoice,
    /// Sobel edge threshold (0.0 - 1.0)
    #[arg(long, default_value_t = 0.2)]
    sobel_threshold: f32,
}

#[derive(Copy, Clone, Debug, ValueEnum)]
enum GradientPreset {
    Detailed,
    Standard,
    Blocks,
    Binary,
}

#[derive(Copy, Clone, Debug, ValueEnum)]
enum EdgeChoice {
    None,
    Sobel,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Preview(args) => preview(args),
        Commands::Convert(args) => convert(args),
        Commands::Animate(args) => animate(args),
    }
}

fn preview(args: PreviewArgs) -> Result<()> {
    let renderer = AsciiRenderer::default();
    let options = args.settings.to_options();
    let layout = LayoutPolicy::FixedColumns(args.width);
    let output = renderer
        .render_path(&args.input, layout, options)
        .with_context(|| format!("failed to render {:?}", args.input))?;

    for row in output.grid.rows() {
        println!("{}", row);
    }

    Ok(())
}

fn convert(args: ConvertArgs) -> Result<()> {
    let renderer = AsciiRenderer::default();
    let options = args.settings.to_options();
    let layout = LayoutPolicy::FixedColumns(args.width);
    let output = renderer
        .render_path(&args.input, layout, options)
        .with_context(|| format!("failed to render {:?}", args.input))?;

    let mut file = File::create(&args.output)
        .with_context(|| format!("failed to create {:?}", args.output))?;
    for row in output.grid.rows() {
        writeln!(file, "{}", row)?;
    }
    Ok(())
}

fn animate(args: AnimateArgs) -> Result<()> {
    let renderer = AsciiRenderer::default();
    let options = args.settings.to_options();
    let layout = LayoutPolicy::FixedColumns(args.width);
    std::fs::create_dir_all(&args.out_dir)
        .with_context(|| format!("failed to create output directory {:?}", args.out_dir))?;

    let frames = load_frames(&args.input)?;
    let progress = ProgressBar::new(frames.len() as u64);
    progress.set_style(
        ProgressStyle::with_template(
            "{spinner} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} frames",
        )
        .unwrap()
        .progress_chars("=> "),
    );

    for (index, frame) in frames.into_iter().enumerate() {
        let dynamic = DynamicImage::ImageRgba8(frame.into_buffer());
        let output = renderer
            .render_image(dynamic, layout, options.clone())
            .with_context(|| format!("failed to render frame {}", index))?;

        let frame_path = args.out_dir.join(format!("frame_{:04}.txt", index));
        let mut file = File::create(&frame_path)
            .with_context(|| format!("failed to create {:?}", frame_path))?;
        for row in output.grid.rows() {
            writeln!(file, "{}", row)?;
        }
        progress.inc(1);
    }

    progress
        .finish_with_message(format!("Frames written to {:?} (fps {:.2})", args.out_dir, args.fps));
    Ok(())
}

fn load_frames(path: &Path) -> Result<Vec<Frame>> {
    if path.is_dir() {
        load_frames_from_directory(path)
    } else {
        let extension = path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_ascii_lowercase())
            .unwrap_or_default();
        if extension == "gif" {
            load_frames_from_gif(path)
        } else {
            let image =
                image::open(path).with_context(|| format!("failed to open image {:?}", path))?;
            let frame = Frame::new(image.into_rgba8());
            Ok(vec![frame])
        }
    }
}

fn load_frames_from_gif(path: &Path) -> Result<Vec<Frame>> {
    let file = File::open(path).with_context(|| format!("failed to open GIF {:?}", path))?;
    let decoder =
        GifDecoder::new(file).with_context(|| format!("failed to decode GIF {:?}", path))?;
    let frames = decoder
        .into_frames()
        .collect_frames()
        .with_context(|| format!("failed to collect frames from {:?}", path))?;
    Ok(frames)
}

fn load_frames_from_directory(path: &Path) -> Result<Vec<Frame>> {
    let mut entries: Vec<PathBuf> = WalkDir::new(path)
        .into_iter()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_file())
        .map(|entry| entry.path().to_path_buf())
        .collect();
    entries.sort();
    if entries.is_empty() {
        anyhow::bail!("no image files found in {:?}", path);
    }

    let mut frames = Vec::with_capacity(entries.len());
    for entry in entries {
        let image =
            image::open(&entry).with_context(|| format!("failed to open image {:?}", entry))?;
        frames.push(Frame::new(image.into_rgba8()));
    }
    Ok(frames)
}

impl RenderSettings {
    fn to_options(&self) -> AsciiOptions {
        let mut options = AsciiOptions::default();
        options.gradient = self.gradient.to_gradient();
        options.brightness = self.brightness;
        options.contrast = self.contrast;
        options.invert = self.invert;
        options.font_aspect = self.font_aspect.max(0.1);
        options.edge_mode = self.edge.to_mode(self);
        options
    }
}

impl GradientPreset {
    fn to_gradient(self) -> Gradient {
        match self {
            GradientPreset::Detailed => Gradient::detailed(),
            GradientPreset::Standard => Gradient::standard(),
            GradientPreset::Blocks => Gradient::blocks(),
            GradientPreset::Binary => Gradient::binary(),
        }
    }
}

impl EdgeChoice {
    fn to_mode(self, settings: &RenderSettings) -> EdgeMode {
        match self {
            EdgeChoice::None => EdgeMode::None,
            EdgeChoice::Sobel => EdgeMode::Sobel { threshold: settings.sobel_threshold },
        }
    }
}
