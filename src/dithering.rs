use image::{GrayImage, Luma};
use imageproc::drawing::draw_filled_circle_mut;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageDithering {
    Threshold,
    FloydSteinberg,
    Atkinson,
    Halftone,
    Bayer,
}

/// Applies Atkinson dithering to a grayscale image buffer in-place.
///
/// - `img`: mutable reference to GrayImage
pub fn atkinson_dither(img: &mut GrayImage) {
    let (width, height) = img.dimensions();
    let raw = img.as_mut();
    for y in 0..height {
        for x in 0..width {
            let idx = (y * width + x) as usize;
            let old_pixel = raw[idx];
            let new_pixel = if old_pixel > 127 { 255 } else { 0 };
            raw[idx] = new_pixel;
            let error = old_pixel as i16 - new_pixel as i16;

            let update_pixel = |x: i32, y: i32, factor: f32, raw: &mut [u8]| {
                if x >= 0 && x < width as i32 && y >= 0 && y < height as i32 {
                    let idx = (y as u32 * width + x as u32) as usize;
                    let new_val = raw[idx] as i16 + (error as f32 * factor) as i16;
                    raw[idx] = new_val.clamp(0, 255) as u8;
                }
            };

            update_pixel(x as i32 + 1, y as i32, 1.0 / 8.0, raw);
            update_pixel(x as i32 + 2, y as i32, 1.0 / 8.0, raw);
            update_pixel(x as i32 - 1, y as i32 + 1, 1.0 / 8.0, raw);
            update_pixel(x as i32, y as i32 + 1, 1.0 / 8.0, raw);
            update_pixel(x as i32 + 1, y as i32 + 1, 1.0 / 8.0, raw);
            update_pixel(x as i32, y as i32 + 2, 1.0 / 8.0, raw);
        }
    }
}

/// Applies Bayer dithering (4x4 matrix) to a grayscale image buffer in-place.
///
/// - `img`: mutable reference to GrayImage
pub fn bayer_dither(img: &mut GrayImage) {
    let (width, height) = img.dimensions();
    // 4x4 Bayer matrix
    const BAYER_MATRIX: [[u8; 4]; 4] =
        [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];
    // The matrix values are in [0, 15]. We scale the pixel intensity to that range.
    const FACTOR: u8 = 16;

    for y in 0..height {
        for x in 0..width {
            let pixel = img.get_pixel_mut(x, y);
            let threshold = BAYER_MATRIX[(y % 4) as usize][(x % 4) as usize];
            let scaled_intensity = pixel[0] / FACTOR;
            if scaled_intensity > threshold {
                pixel[0] = 255; // White
            } else {
                pixel[0] = 0; // Black
            }
        }
    }
}

/// Applies halftone dithering to a grayscale image, returning a new image.
///
/// - `img`: reference to GrayImage
///
/// Returns a new GrayImage with halftone effect
pub fn halftone_dither(img: &GrayImage) -> GrayImage {
    let (width, height) = img.dimensions();
    let side = 4;
    let jump = 4;
    let alpha = 3.0;

    let height_output = side * ((height as f32 / jump as f32).ceil() as u32);
    let width_output = side * ((width as f32 / jump as f32).ceil() as u32);
    let mut canvas = GrayImage::new(width_output, height_output);

    let mut y_output = 0;
    for y in (0..height).step_by(jump) {
        let mut x_output = 0;
        for x in (0..width).step_by(jump) {
            let mut sum = 0.0;
            let mut n = 0;
            for dy in 0..jump {
                for dx in 0..jump {
                    if x + (dx as u32) < width && y + (dy as u32) < height {
                        sum += img.get_pixel(x + dx as u32, y + dy as u32)[0] as f32;
                        n += 1;
                    }
                }
            }
            let avg = sum / n as f32;
            let intensity = 1.0 - avg / 255.0;
            let radius = (alpha * intensity * side as f32 / 2.0) as i32;

            if radius > 0 {
                let mut square = GrayImage::from_pixel(side, side, Luma([255]));
                draw_filled_circle_mut(
                    &mut square,
                    (side as i32 / 2, side as i32 / 2),
                    radius,
                    Luma([0]),
                );
                image::imageops::overlay(&mut canvas, &square, x_output as i64, y_output as i64);
            } else {
                let square = GrayImage::from_pixel(side, side, Luma([255]));
                image::imageops::overlay(&mut canvas, &square, x_output as i64, y_output as i64);
            }
            x_output += side;
        }
        y_output += side;
    }
    canvas
}
