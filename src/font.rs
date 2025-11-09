use once_cell::sync::Lazy;
use rusttype::{point, Font, PositionedGlyph, Scale};

static FONT_DATA: &[u8] = include_bytes!("../Frisky Puppy.ttf");
static FONT: Lazy<Font<'static>> =
    Lazy::new(|| Font::try_from_bytes(FONT_DATA).expect("Failed to load embedded TTF)"));

/// Rasterizes text into a grayscale pixel buffer for printing.
///
/// - `text`: The text to render (supports multiline)
/// - `width`: Output image width in pixels
/// - `font_size`: Font size in points
///
/// Returns a Vec<u8> (row-major, 0=black, 255=white)
pub fn rasterize_text(text: &str, width: usize, font_size: f32) -> Vec<u8> {
    let scale = Scale::uniform(font_size);
    let v_metrics = FONT.v_metrics(scale);
    let line_height = (v_metrics.ascent - v_metrics.descent + v_metrics.line_gap).ceil() as usize;
    if width == 0 {
        return vec![];
    }

    let mut lines: Vec<String> = Vec::new();
    for raw_line in text.lines() {
        let mut cur = String::new();
        for word in raw_line.split_whitespace() {
            if cur.is_empty() {
                cur.push_str(word);
            } else {
                let trial = format!("{} {}", cur, word);
                if text_pixel_width(&trial, scale) <= width as f32 {
                    cur = trial;
                } else {
                    lines.push(cur);
                    cur = word.to_string();
                }
            }
        }
        lines.push(cur);
    }

    let height = if lines.is_empty() {
        line_height
    } else {
        lines.len() * line_height
    };

    let mut pixels = vec![255u8; width * height];

    for (line_idx, line) in lines.iter().enumerate() {
        let y_baseline = (line_idx * line_height) as f32 + v_metrics.ascent;
        let glyphs: Vec<PositionedGlyph> =
            FONT.layout(line, scale, point(0.0, y_baseline)).collect();
        for glyph in glyphs {
            if let Some(bb) = glyph.pixel_bounding_box() {
                glyph.draw(|gx, gy, v| {
                    let px = bb.min.x + gx as i32;
                    let py = bb.min.y + gy as i32;
                    if px >= 0 && py >= 0 {
                        let ux = px as usize;
                        let uy = py as usize;
                        if ux < width && uy < height && v > 0.3 {
                            pixels[uy * width + ux] = 0;
                        }
                    }
                });
            }
        }
    }

    pixels
}

fn text_pixel_width(s: &str, scale: Scale) -> f32 {
    let mut w = 0.0f32;
    for ch in s.chars() {
        let g = FONT.glyph(ch).scaled(scale);
        let h = g.h_metrics().advance_width;
        w += h;
    }
    w
}
