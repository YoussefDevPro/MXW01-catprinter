use crate::dithering::{atkinson_dither, bayer_dither, halftone_dither, ImageDithering};
use crate::protocol::*;
use std::time::Duration;

/// Transport trait for CatPrinter communication (sync).
/// Implement this for your BLE or mock transport.
pub trait Transport {
    /// Write a control packet to the printer.
    fn write_control(&mut self, data: &[u8]) -> Result<(), String>;
    /// Write image/text data to the printer.
    fn write_data(&mut self, data: &[u8]) -> Result<(), String>;
    /// Read a notification from the printer (with timeout).
    fn read_notification(&mut self, timeout: Duration) -> Result<Vec<u8>, String>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrinterState {
    Standby,
    Printing,
    Error(u8),
    Unknown,
}

#[derive(Debug, Clone)]
pub struct PrinterStatus {
    pub battery_percent: Option<u8>,
    pub temperature: Option<u8>,
    pub state: PrinterState,
}

/// Synchronous CatPrinter API for printing text and images.
///
/// - `transport`: implements Transport trait (BLE or mock)
/// - `chunk_size`: bytes per data chunk (default: 180)
pub struct CatPrinter<T: Transport> {
    pub transport: T,
    pub chunk_size: usize,
}

impl<T: Transport> CatPrinter<T> {
    pub fn new(transport: T) -> Self {
        Self {
            transport,
            chunk_size: 180,
        }
    }

    /// Query the printer for its current status (battery, temperature, state).
    ///
    /// - `timeout`: max time to wait for response
    ///
    /// Returns PrinterStatus struct
    pub fn get_status(&mut self, timeout: Duration) -> Result<PrinterStatus, String> {
        let req = build_control_packet(0xA1, &[0x00]);
        self.transport.write_control(&req)?;
        let raw = self.transport.read_notification(timeout)?;
        let notif = parse_notification(&raw).map_err(|e| e.to_string())?;
        Ok(parse_printer_status(&notif.payload))
    }

    /// Print text to the CatPrinter (with author signature).
    ///
    /// - `main`: main text to print
    /// - `author`: author name
    ///
    /// Returns Ok(()) on success
    pub fn print_text(&mut self, main: &str, author: &str) -> Result<(), String> {
        let width = 384usize;
        let pixels = render_text_to_pixels(main, author, width);
        let height = pixels.len() / width;
        // Rotate and mirror text buffer for CatPrinter
        let rotated_pixels = crate::protocol::rotate_mirror_pixels(&pixels, width, height);
        let packed = pack_1bpp_pixels(&rotated_pixels, width, height).map_err(|e| e.to_string())?;

        let line_count: u16 = height as u16;
        let mut a9_payload = Vec::new();
        a9_payload.extend_from_slice(&line_count.to_le_bytes());
        a9_payload.push(0x30);
        a9_payload.push(0x00); // mode 0 = 1bpp
        let a9 = build_control_packet(0xA9, &a9_payload);
        self.transport.write_control(&a9)?;
        let resp = self.transport.read_notification(Duration::from_secs(2))?;
        let parsed = parse_notification(&resp).map_err(|e| e.to_string())?;
        if parsed.command_id != 0xA9 || parsed.payload.first() == Some(&0x01u8) {
            return Err("printer rejected print request".into());
        }

        let chunks = chunk_data(&packed, self.chunk_size);
        for chunk in chunks {
            self.transport.write_data(chunk)?;
        }

        let ad = build_control_packet(0xAD, &[0x00]);
        self.transport.write_control(&ad)?;

        let deadline = std::time::Instant::now() + Duration::from_secs(60);
        loop {
            let timeout = deadline
                .checked_duration_since(std::time::Instant::now())
                .unwrap_or_else(|| Duration::from_secs(0));
            if timeout.is_zero() {
                return Err("timed out waiting for print complete".into());
            }
            let raw = self.transport.read_notification(timeout)?;
            let notif = parse_notification(&raw).map_err(|e| e.to_string())?;
            if notif.command_id == 0xAA {
                return Ok(());
            }
        }
    }

    /// Print an image from a file path, with improved clarity and correctness.
    /// Steps:
    /// 1. Load image
    /// 2. Convert to grayscale
    /// 3. Resize/crop to printer width and reasonable height
    /// 4. Optionally rotate/flip for correct orientation
    /// 5. Apply dithering
    /// 6. Pack pixels and send to printer
    /// Print an image from a file path, with optional dithering.
    ///
    /// - `path`: path to image file
    /// - `dithering`: dithering algorithm to apply
    ///
    /// Returns Ok(()) on success
    pub fn print_image_from_path(&mut self, path: &str, dithering: ImageDithering) -> Result<(), String> {
        // 1. Load image
        let img = image::open(path).map_err(|e| e.to_string())?;
        let printer_width = 384;
        let max_height = 800; // reasonable max height for most prints

        // 2. Convert to grayscale
        let gray = img.to_luma8();

        // 3. Resize/crop to printer width and max height, center vertically if needed
        let (orig_w, orig_h) = gray.dimensions();
        let scale = printer_width as f32 / orig_w as f32;
        let target_h = ((orig_h as f32) * scale).min(max_height as f32) as u32;
        let resized = image::imageops::resize(&gray, printer_width, target_h, image::imageops::FilterType::Lanczos3);
        let mut gray = resized;
// Now gray is the resized grayscale image, ready for orientation and dithering.

        // 4. Optionally rotate/flip for correct orientation
        // Uncomment one of the following lines if your prints are upside down or sideways:
        // gray = image::imageops::rotate90(&gray);
        // gray = image::imageops::rotate180(&gray);
        // gray = image::imageops::flip_vertical(&gray);
        // gray = image::imageops::flip_horizontal(&gray);

        // 5. Apply dithering
        match dithering {
            ImageDithering::FloydSteinberg => {
                image::imageops::dither(&mut gray, &image::imageops::BiLevel);
            }
            ImageDithering::Atkinson => {
                atkinson_dither(&mut gray);
            }
            ImageDithering::Bayer => {
                bayer_dither(&mut gray);
            }
            ImageDithering::Halftone => {
                gray = halftone_dither(&gray);
            }
            ImageDithering::Threshold => {
                for pixel in gray.pixels_mut() {
                    pixel[0] = if pixel[0] > 127 { 255 } else { 0 };
                }
            }
        }

        // 6. Save processed image for debugging
        let _ = gray.save("processed_for_print.png"); // Save to disk for visual inspection

        // 7. Pack pixels and send to printer
        let (width, height) = gray.dimensions();
        let pixels = gray.as_raw();
        self.print_image(pixels, width as usize, height as usize, 0x00, None)
    }

    /// Print a raw grayscale pixel buffer as an image.
    ///
    /// - `pixels`: grayscale buffer (row-major, 0=black, 255=white)
    /// - `width`, `height`: image dimensions
    /// - `mode`: print mode (0x00 = 1bpp)
    /// - `chunk_size`: optional override for data chunk size
    ///
    /// Returns Ok(()) on success
    pub fn print_image(
        &mut self,
        pixels: &[u8],
        width: usize,
        height: usize,
        mode: u8,
        chunk_size: Option<usize>,
    ) -> Result<(), String> {
        let packed = pack_1bpp_pixels(pixels, width, height).map_err(|e| e.to_string())?;

        // Send A9
        let line_count: u16 = height as u16;
        let mut a9_payload = Vec::new();
        a9_payload.extend_from_slice(&line_count.to_le_bytes());
        a9_payload.push(0x30);
        a9_payload.push(mode);
        let a9 = build_control_packet(0xA9, &a9_payload);
        self.transport.write_control(&a9)?;
        let resp = self.transport.read_notification(Duration::from_secs(2))?;
        let parsed = parse_notification(&resp).map_err(|e| e.to_string())?;
        if parsed.command_id != 0xA9 || parsed.payload.first() == Some(&0x01u8) {
            return Err("printer rejected print request".into());
        }

        let size = chunk_size.unwrap_or(self.chunk_size);
        let chunks = chunk_data(&packed, size);
        for chunk in chunks {
            self.transport.write_data(chunk)?;
        }
        let ad = build_control_packet(0xAD, &[0x00]);
        self.transport.write_control(&ad)?;

        let deadline = std::time::Instant::now() + Duration::from_secs(60);
        loop {
            let timeout = deadline
                .checked_duration_since(std::time::Instant::now())
                .unwrap_or_else(|| Duration::from_secs(0));
            if timeout.is_zero() {
                return Err("timed out waiting for print complete".into());
            }
            let raw = self.transport.read_notification(timeout)?;
            let notif = parse_notification(&raw).map_err(|e| e.to_string())?;
            if notif.command_id == 0xAA {
                return Ok(());
            }
        }
    }
}

