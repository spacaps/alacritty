use image::DynamicImage;
use image::GenericImageView;

pub trait FrameSource {
    fn dimensions(&self) -> (u32, u32);
    fn next_frame(&mut self) -> Option<DynamicImage>;
}

pub struct StaticFrame {
    image: DynamicImage,
}

impl StaticFrame {
    pub fn new(image: DynamicImage) -> Self {
        Self { image }
    }
}

impl FrameSource for StaticFrame {
    fn dimensions(&self) -> (u32, u32) {
        self.image.dimensions()
    }

    fn next_frame(&mut self) -> Option<DynamicImage> {
        Some(self.image.clone())
    }
}
