use std::io::{self, Write};
use std::time::Duration;

use tokio;

use catprinter::ble::{connect, scan};

/// Example: Query CatPrinter status and battery in a loop
/// - Scans for BLE printers
/// - Lets user select device
/// - Queries status/battery 10 times
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Scanning for CatPrinter-compatible BLE devices for 3 seconds...");
    let devices = scan(Duration::from_secs(3)).await?;
    if devices.is_empty() {
        println!(
            "No devices found. Make sure your Bluetooth adapter is up and the printer is powered on and advertising."
        );
        return Ok(());
    }

    println!("Found devices:");
    for (i, d) in devices.iter().enumerate() {
        println!("  {}) id={} name={:?}", i + 1, d.id, d.name);
    }

    // Ask user to pick a device
    let mut input = String::new();
    let chosen = loop {
        print!("Select device number to connect to (1-{}): ", devices.len());
        io::stdout().flush()?;
        input.clear();
        io::stdin().read_line(&mut input)?;
        if let Ok(n) = input.trim().parse::<usize>() {
            if n >= 1 && n <= devices.len() {
                break &devices[n - 1];
            }
        }
        println!("Invalid selection.");
    };

    println!("Connecting to device id={} name={:?} ...", chosen.id, chosen.name);
    let printer = match connect(&chosen.id, Duration::from_secs(10)).await {
        Ok(p) => {
            println!("Connected successfully.");
            p
        }
        Err(e) => {
            eprintln!("Failed to connect: {}", e);
            return Ok(());
        }
    };

    // Query status and battery 10 times, 1s apart
    println!("Starting status/battery query loop (10 iterations, 1s apart)...");
    for i in 0..10 {
        println!("Query {}:", i + 1);
        match printer.get_status(Duration::from_secs(10)).await {
            Ok(s) => {
                println!(
                    "Status (0xA1) -> battery: {:?}, temperature: {:?}, state: {:?}",
                    s.battery_percent, s.temperature, s.state
                );
            }
            Err(e) => {
                eprintln!("Failed to get status: {}", e);
            }
        }
        match printer.get_battery(Duration::from_secs(10)).await {
            Ok(b) => {
                println!("Battery (0xAB) -> battery percent: {}", b);
            }
            Err(e) => {
                eprintln!("Failed to get battery: {}", e);
            }
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
    println!("Status/battery query loop finished.");
    Ok(())
}
