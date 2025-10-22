use image::DynamicImage;

pub fn extract_luma(image: &DynamicImage, invert: bool) -> Vec<f32> {
    let gray = image.to_luma32f();
    let mut data = Vec::with_capacity((gray.width() * gray.height()) as usize);
    for pixel in gray.pixels() {
        let mut lum = pixel.0[0];
        if invert {
            lum = 1.0 - lum;
        }
        data.push(lum.clamp(0.0, 1.0));
    }
    data
}

pub fn apply_contrast_and_brightness(values: &mut [f32], contrast: f32, brightness: f32) {
    if contrast == 0.0 && brightness == 0.0 {
        return;
    }

    let contrast = contrast.clamp(-255.0, 255.0);
    let contrast_factor = (259.0 * (contrast + 255.0)) / (255.0 * (259.0 - contrast));
    let brightness = (brightness / 255.0).clamp(-1.0, 1.0);

    for value in values {
        let mut v = *value;
        v = contrast_factor * (v - 0.5) + 0.5 + brightness;
        *value = v.clamp(0.0, 1.0);
    }
}
