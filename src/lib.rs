//! CatPrinter library: print text and images to CatPrinter devices via BLE.
//!
//! Main modules:
//! - ble: BLE transport and async printer
//! - dithering: image dithering algorithms
//! - font: text rasterization
//! - printer: sync printer
//! - protocol: packet and data utilities

pub mod ble;
pub mod dithering;
pub mod font;
pub mod printer;
pub mod protocol;

/// BLE API: scan/connect to printers, async printing
pub use ble::{connect, scan, CatPrinterAsync, DeviceInfo};
/// Sync printer API
pub use printer::*;
/// Protocol utilities (packets, pixel packing, etc)
pub use protocol::*;
