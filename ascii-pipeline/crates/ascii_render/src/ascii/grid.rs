#[derive(Clone, Debug)]
pub struct CellGlyph {
    pub ch: char,
    /// Foreground color encoded as RGB bytes.
    pub fg: [u8; 3],
    /// Optional background color encoded as RGB bytes.
    pub bg: Option<[u8; 3]>,
    /// Transparency multiplier (1.0 = fully opaque glyph color).
    pub alpha: f32,
}

impl CellGlyph {
    pub fn new(ch: char, intensity: f32) -> Self {
        let intensity = (intensity.clamp(0.0, 1.0) * 255.0).round() as u8;
        Self { ch, fg: [intensity; 3], bg: None, alpha: 1.0 }
    }
}

#[derive(Clone, Debug)]
pub struct GlyphGrid {
    pub width: u16,
    pub height: u16,
    pub cells: Vec<CellGlyph>,
}

impl GlyphGrid {
    pub fn new(width: u16, height: u16, cells: Vec<CellGlyph>) -> Self {
        assert_eq!(usize::from(width) * usize::from(height), cells.len());
        Self { width, height, cells }
    }

    pub fn rows(&self) -> impl Iterator<Item = String> + '_ {
        let width = self.width as usize;
        self.cells.chunks(width).map(|row| row.iter().map(|cell| cell.ch).collect::<String>())
    }
}
