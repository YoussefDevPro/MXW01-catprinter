use crate::font;

/// Computes CRC-8 for a byte slice (CatPrinter protocol).
///
/// - `data`: input bytes
///
/// Returns CRC-8 value
pub fn crc8(data: &[u8]) -> u8 {
    let poly: u8 = 0x07;
    let mut crc: u8 = 0x00;

    for &b in data {
        crc ^= b;
        for _ in 0..8 {
            if (crc & 0x80) != 0 {
                crc = (crc << 1) ^ poly;
            } else {
                crc <<= 1;
            }
        }
    }
    crc
}

/// Builds a control packet for the CatPrinter protocol.
/// Returns a Vec<u8> ready to send.
/// Builds a control packet for the CatPrinter protocol.
///
/// - `command_id`: command byte
/// - `payload`: command payload
///
/// Returns Vec<u8> ready to send
pub fn build_control_packet(command_id: u8, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(2 + 1 + 1 + 2 + payload.len() + 1 + 1);
    out.push(0x22);
    out.push(0x21);
    out.push(command_id);
    out.push(0x00);
    let len = payload.len() as u16;
    out.push((len & 0xFF) as u8);
    out.push((len >> 8) as u8);
    out.extend_from_slice(payload);
    out.push(crc8(payload));
    out.push(0xFF);
    out
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notification {
    pub command_id: u8,
    pub unknown: u8,
    pub payload: Vec<u8>,
    pub crc: Option<u8>,
}

/// Parses a notification packet from the CatPrinter.
/// Parses a notification packet from the CatPrinter.
///
/// - `data`: raw notification bytes
///
/// Returns Notification struct on success
pub fn parse_notification(data: &[u8]) -> Result<Notification, &'static str> {
    if data.len() < 7 {
        return Err("packet too short");
    }
    if data[0] != 0x22 || data[1] != 0x21 {
        return Err("bad preamble");
    }
    let cmd = data[2];
    let unknown = data[3];
    let len_lo = data[4] as usize;
    let len_hi = data[5] as usize;
    let payload_len = (len_hi << 8) | len_lo;
    if data.len() < 6 + payload_len {
        return Err("not enough bytes for claimed payload length");
    }
    let payload = data[6..6 + payload_len].to_vec();
    let crc = data.get(6 + payload_len).copied();
    Ok(Notification {
        command_id: cmd,
        unknown,
        payload,
        crc,
    })
}

/// Pack pixels given as bytes (0 = white, non-zero = black) row-major.
/// width must be >0. Returns bytes in the printer's expected layout:
/// - rows top->bottom
/// - within each row, groups of 8 pixels become one byte where bit 0 = leftmost pixel of group.
/// Packs a grayscale image buffer into 1bpp format for CatPrinter.
/// - 0 = black, non-zero = white
/// - Bits are packed LSB-first (bit 0 = leftmost pixel)
/// - Rows are packed top-to-bottom, left-to-right
/// Packs a grayscale image buffer into 1bpp format for CatPrinter.
///
/// - `pixels`: grayscale buffer (row-major, 0=black, 255=white)
/// - `width`, `height`: image dimensions
///
/// Returns packed bytes in printer's expected layout
pub fn pack_1bpp_pixels(
    pixels: &[u8],
    width: usize,
    height: usize,
) -> Result<Vec<u8>, &'static str> {
    if width == 0 || height == 0 {
        return Err("width/height must be > 0");
    }
    let required = width.checked_mul(height).ok_or("width*height overflow")?;
    if pixels.len() < required {
        return Err("not enough pixels");
    }
    let bytes_per_row = (width + 7) / 8;
    let mut out = Vec::with_capacity(bytes_per_row * height);
    for row in 0..height {
        let row_off = row * width;
        for group in 0..bytes_per_row {
            let mut b: u8 = 0;
            let base = row_off + group * 8;
            let end = usize::min(base + 8, row_off + width);
            for (bit, px_idx) in (base..end).enumerate() {
                let bit_val = if pixels[px_idx] == 0 { 1u8 } else { 0u8 };
                b |= bit_val << bit;
            }
            out.push(b);
        }
    }
    Ok(out)
}

use crate::printer::{PrinterState, PrinterStatus};

/// Parses the payload bytes from a CatPrinter notification into a PrinterStatus struct.
///
/// Payload mapping:
/// - payload[6]: status flag (0 = Standby, 1 = Printing, other = Unknown)
/// - payload[9]: battery percent (if available)
/// - payload[10]: temperature (if available)
/// - payload[12]: overall flag (if nonzero and payload.len() > 13, error)
/// - payload[13]: error code (if error)
///
/// Returns PrinterStatus with battery, temperature, and state.
pub fn parse_printer_status(payload: &[u8]) -> PrinterStatus {
    let mut battery = None;
    let mut temp = None;
    let mut state = PrinterState::Unknown;
    if payload.len() >= 13 {
        let overall_flag = payload[12];
        let status_flag = payload[6];
        if overall_flag == 0 {
            state = match status_flag {
                0 => PrinterState::Standby,
                1 => PrinterState::Printing,
                _ => PrinterState::Unknown,
            };
        } else if payload.len() > 13 {
            state = PrinterState::Error(payload[13]);
        }
        battery = Some(payload[9]);
        temp = Some(payload[10]);
    }
    PrinterStatus {
        battery_percent: battery,
        temperature: temp,
        state,
    }
}

/// Splits data into chunks of given size.
///
/// - `data`: input bytes
/// - `chunk_size`: size of each chunk
///
/// Returns Vec of byte slices
pub fn chunk_data(data: &[u8], chunk_size: usize) -> Vec<&[u8]> {
    if chunk_size == 0 {
        return vec![data];
    }
    data.chunks(chunk_size).collect()
}

/// Rotates and mirrors a pixel buffer for CatPrinter (180° rotation).
/// Input: pixels (row-major, 0=black, 255=white), width, height
/// Output: rotated and mirrored pixel buffer
pub fn rotate_mirror_pixels(pixels: &[u8], width: usize, height: usize) -> Vec<u8> {
    let mut rotated = vec![0u8; pixels.len()];
    for row in 0..height {
        for col in 0..width {
            let src_idx = row * width + col;
            let dst_idx = (height - 1 - row) * width + (width - 1 - col);
            rotated[dst_idx] = pixels[src_idx];
        }
    }
    rotated
}

/// Renders text and author signature to a grayscale pixel buffer for printing.
///
/// - `main`: main text
/// - `author`: author name
/// - `width`: output image width
///
/// Returns Vec<u8> (row-major, 0=black, 255=white)
pub fn render_text_to_pixels(main: &str, author: &str, width: usize) -> Vec<u8> {
    let mut full = String::new();
    full.push_str(main);
    full.push('\n');
    full.push('\n');
    full.push_str("-- ");
    full.push_str(author);

    let font_size = 24.0_f32 * 2.0;

    font::rasterize_text(&full, width, font_size)
}
