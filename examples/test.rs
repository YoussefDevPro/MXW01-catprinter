use std::io::{self, Write};
use std::time::Duration;

use catprinter::ble::{connect, scan};

/// Example: Interactive CatPrinter session
/// - Scans for BLE printers
/// - Lets user select device
/// - Prints text or image based on user input
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
    loop {
        print!("Select device number to connect to (1-{}): ", devices.len());
        io::stdout().flush()?;
        input.clear();
        io::stdin().read_line(&mut input)?;
        if let Ok(n) = input.trim().parse::<usize>() {
            if n >= 1 && n <= devices.len() {
                let chosen = &devices[n - 1];
                println!(
                    "Connecting to device id={} name={:?} ...",
                    chosen.id, chosen.name
                );
                match connect(&chosen.id, Duration::from_secs(10)).await {
                    Ok(printer) => {
                        println!("Connected successfully.");
                        run_interactive_session(printer).await?;
                        return Ok(());
                    }
                    Err(e) => {
                        eprintln!("Failed to connect: {}", e);
                        // Ask to retry or choose another device
                        print!("Try another device? (y/N): ");
                        io::stdout().flush()?;
                        input.clear();
                        io::stdin().read_line(&mut input)?;
                        if input.trim().to_lowercase() != "y" {
                            return Ok(());
                        } else {
                            // re-list and continue loop to let user enter another index
                            for (i, d) in devices.iter().enumerate() {
                                println!("  {}) id={} name={:?}", i + 1, d.id, d.name);
                            }
                        }
                    }
                }
                break;
            }
        }
        println!("Invalid selection.");
    }

    Ok(())
}

/// Runs an interactive print session with the selected CatPrinter.
/// Prompts user for text or image, then prints.
async fn run_interactive_session(
    printer: catprinter::ble::CatPrinterAsync,
) -> Result<(), Box<dyn std::error::Error>> {
    // Show status
    println!("Querying printer status...");
    match printer.get_status(Duration::from_secs(10)).await {
        Ok(s) => {
            println!(
                "Status -> battery: {:?}, temperature: {:?}, state: {:?}",
                s.battery_percent, s.temperature, s.state
            );
        }
        Err(e) => {
            eprintln!("Failed to get status: {}", e);
        }
    }
    let mut mode = String::new();
    // Prompt user for print mode
    print!("Choose print mode: 1 for text, 2 for image: ");
    io::stdout().flush()?;
    io::stdin().read_line(&mut mode)?;

    if mode.trim() == "1" {
        // Prompt user for text and author
        let mut main = String::new();
        let mut author = String::new();
        // Prompt for text and author
        print!("Enter the main text to print: ");
        io::stdout().flush()?;
        io::stdin().read_line(&mut main)?;
        print!("Enter the author name: ");
        io::stdout().flush()?;
        io::stdin().read_line(&mut author)?;
        let main = main.trim();
        let author = author.trim();

        if main.is_empty() {
            println!("No text entered, aborting.");
            return Ok(());
        }

        // Print text
        println!("Sending print job (text)...");
        match printer.print_text(main, author).await {
            Ok(()) => println!("Print job completed (printer reported AA/complete)."),
            Err(e) => eprintln!("Print job failed: {}", e),
        }
    } else if mode.trim() == "2" {
        let mut img_path = String::new();
        // Prompt for image path
        print!("Enter the path to the image: ");
        io::stdout().flush()?;
        io::stdin().read_line(&mut img_path)?;
        let img_path = img_path.trim();

        if img_path.is_empty() {
            println!("No image path entered, aborting.");
            return Ok(());
        }

        // Prompt for dithering mode
        let mut dithering_mode = String::new();
        print!("Choose dithering mode: 1=Threshold, 2=Floyd-Steinberg, 3=Atkinson, 4=Halftone: ");
        io::stdout().flush()?;
        io::stdin().read_line(&mut dithering_mode)?;
        let dithering = match dithering_mode.trim() {
            "2" => catprinter::dithering::ImageDithering::FloydSteinberg,
            "3" => catprinter::dithering::ImageDithering::Atkinson,
            "4" => catprinter::dithering::ImageDithering::Halftone,
            _ => catprinter::dithering::ImageDithering::Threshold,
        };

        // Print image
        println!("Sending print job (image)...");
        match printer.print_image_from_path(img_path, dithering).await {
            Ok(()) => println!("Print job completed (printer reported AA/complete)."),
            Err(e) => eprintln!("Print job failed: {}", e),
        }
    } else {
        println!("Invalid selection.");
    }

    Ok(())
}
