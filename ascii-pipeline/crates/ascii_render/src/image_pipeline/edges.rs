use std::cmp::{max, min};

#[derive(Clone, Copy, Debug)]
pub enum EdgeMode {
    None,
    Sobel { threshold: f32 },
}

#[derive(Clone, Debug)]
pub struct EdgeSample {
    pub active: bool,
    pub magnitude: f32,
    pub angle_degrees: f32,
}

pub enum EdgeResult {
    Intensity(Vec<f32>),
    Orientation(Vec<EdgeSample>),
}

pub fn sobel_map(values: &[f32], width: u16, height: u16, threshold: f32) -> Vec<f32> {
    let width = width as usize;
    let height = height as usize;
    let mut output = vec![0.0; values.len()];
    let threshold = threshold.clamp(0.0, 1.0);

    if width < 3 || height < 3 {
        return output;
    }

    for y in 1..height - 1 {
        for x in 1..width - 1 {
            let idx = y * width + x;

            let a = values[(y - 1) * width + (x - 1)];
            let b = values[(y - 1) * width + x];
            let c = values[(y - 1) * width + (x + 1)];
            let d = values[y * width + (x - 1)];
            let f = values[y * width + (x + 1)];
            let g = values[(y + 1) * width + (x - 1)];
            let h = values[(y + 1) * width + x];
            let i = values[(y + 1) * width + (x + 1)];

            let gx = (-1.0 * a) + (1.0 * c) + (-2.0 * d) + (2.0 * f) + (-1.0 * g) + (1.0 * i);
            let gy = (-1.0 * a) + (-2.0 * b) + (-1.0 * c) + (1.0 * g) + (2.0 * h) + (1.0 * i);
            let magnitude = (gx * gx + gy * gy).sqrt();
            let normalized = (magnitude / 4.0).clamp(0.0, 1.0);
            output[idx] = if normalized >= threshold { normalized } else { 0.0 };
        }
    }

    output
}

fn sobel_with_angle(data: &[Vec<f32>]) -> (Vec<Vec<f32>>, Vec<Vec<f32>>) {
    let height = data.len();
    let width = data[0].len();
    let mut magnitude = vec![vec![0.0f32; width]; height];
    let mut angle = vec![vec![0.0f32; width]; height];

    for y in 1..height - 1 {
        for x in 1..width - 1 {
            let a = data[y - 1][x - 1];
            let b = data[y - 1][x];
            let c = data[y - 1][x + 1];
            let d = data[y][x - 1];
            let f = data[y][x + 1];
            let g = data[y + 1][x - 1];
            let h = data[y + 1][x];
            let i = data[y + 1][x + 1];

            let gx = (-1.0 * a) + (1.0 * c) + (-2.0 * d) + (2.0 * f) + (-1.0 * g) + (1.0 * i);
            let gy = (-1.0 * a) + (-2.0 * b) + (-1.0 * c) + (1.0 * g) + (2.0 * h) + (1.0 * i);
            let mag = (gx * gx + gy * gy).sqrt();
            let mut theta = gy.atan2(gx).to_degrees();
            if theta < 0.0 {
                theta += 180.0;
            }
            magnitude[y][x] = mag;
            angle[y][x] = theta;
        }
    }

    (magnitude, angle)
}
