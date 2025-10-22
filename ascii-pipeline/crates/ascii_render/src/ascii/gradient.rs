#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Gradient {
    chars: Vec<char>,
}

impl Gradient {
    pub fn new(chars: impl Into<String>) -> Self {
        let chars: Vec<char> = chars.into().chars().collect();
        assert!(chars.len() >= 2, "gradient must contain at least two characters");
        Self { chars }
    }

    pub fn detailed() -> Self {
        Self::new("$@B%8&WM#*oahkbdpqwmZO0QLCJUYXzcvunxrjft/\\|()1{}[]?-_+~<>i!lI;:,\"^`'. ")
    }

    pub fn standard() -> Self {
        Self::new("@%#*+=-:. ")
    }

    pub fn blocks() -> Self {
        Self::new("█▓▒░ ")
    }

    pub fn binary() -> Self {
        Self::new("01")
    }

    pub fn len(&self) -> usize {
        self.chars.len()
    }

    pub fn chars(&self) -> &[char] {
        &self.chars
    }

    pub fn clamp_index(&self, value: f32) -> usize {
        let levels = (self.chars.len() - 1) as f32;
        let idx = (value * levels).clamp(0.0, levels);
        idx.round() as usize
    }

    pub fn char_at(&self, index: usize) -> char {
        self.chars[index.min(self.chars.len() - 1)]
    }
}
