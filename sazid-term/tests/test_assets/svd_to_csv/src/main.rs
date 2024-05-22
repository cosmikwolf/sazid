use clap::{command, Parser};
use csv::Writer;
use std::{fs::File, io::Read, path::PathBuf};
use svd_parser::svd::Device;
use svd_parser::ValidateLevel;

/// Simple program to greet a person
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Output file path
    #[arg(short, long)]
    output: Option<String>,

    /// Input SVD file path
    svd_path: std::path::PathBuf,
}

fn main() {
    let cli = Args::parse();
    let svd_path = cli.svd_path;
    let out_path = match cli.output {
        Some(path) => PathBuf::from(path),
        None => {
            let stem = svd_path.file_stem().unwrap().to_str().unwrap();
            let new_name = format!("./{}.csv", stem);
            PathBuf::from(new_name)
        }
    };

    // Load SVD file
    let mut file = File::open(svd_path.clone()).expect("Could not open SVD file");
    let mut contents = String::new();
    file.read_to_string(&mut contents)
        .expect("Could not read SVD file");

    // Parse SVD file
    let mut parser_config = svd_parser::Config::default();
    parser_config.validate_level = ValidateLevel::Weak;
    parser_config.ignore_enums(true);
    parser_config.expand(true);
    parser_config.expand_properties(true);

    let mut device = svd_parser::parse_with_config(&contents, &parser_config)
        .expect("Error parsing SVD XML file");

    // Create a CSV writer
    let mut wtr = Writer::from_path(out_path.clone()).expect("Could not create CSV file");

    // Iterate over peripherals
    write_peripheral_to_csv(&mut wtr, device).expect("Could not write peripheral details to CSV");

    wtr.flush().expect("Failed to flush CSV writer");

    println!(
        "The SVD file '{}' has been successfully processed into '{}'",
        svd_path.display(),
        out_path.display()
    );
}

fn write_peripheral_to_csv(writer: &mut csv::Writer<File>, device: Device) -> csv::Result<()> {
    // Assuming `svd_data` is a string that contains your SVD XML.
    // You would likely load this from a file or another source in a real application.
    // Write CSV header
    writer
        .write_record(["Peripheral Name", "Register Name", "Address Offset"])
        .expect("Could write CSV header");

    for peripheral in device.peripherals {
        // You will need to adjust this based on the actual fields you need and what's available on your `Peripheral` type
        let description = match peripheral.description.clone() {
            Some(desc) => desc,
            None => "".to_string(),
        };
        println!(
            "{}\t{}\t{}",
            &peripheral.name,
            &description,
            &peripheral.base_address.to_string(),
        );
        writer.write_record([
            &peripheral.name,
            &description,
            &peripheral.base_address.to_string(),
        ])?;
    }

    writer.flush()?;
    Ok(())
}
