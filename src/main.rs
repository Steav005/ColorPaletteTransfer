use argparse::{ArgumentParser, Store};
use atomic_counter::AtomicCounter;
use atomic_counter::RelaxedCounter;
use image::io::Reader as ImageReader;
use image::save_buffer_with_format;
use image::ImageFormat;
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use std::sync::{Arc, RwLock};
use std::thread;
use std::thread::sleep;
use std::time::Duration;

use color_palatte_transfer::*;

fn main() -> Result<(), TransferError> {
    let mut output = String::from("");
    let mut colors = String::from("");
    let mut image = String::from("");
    {
        let mut ap = ArgumentParser::new();
        ap.set_description("Converts image to color palette");
        ap.refer(&mut output).add_option(
            &["-o", "--output"],
            Store,
            "Set output name. \
            Tries to honour set extension. \
            Example: output.png",
        );
        ap.refer(&mut colors).add_option(
            &["-c", "--colors"],
            Store,
            "Hexcodes in parenthesis and split by comma. \
            Example: \"2E3440,3B4252,434C5E\". \
            Uses Nord color palette if not set: https://www.nordtheme.com/",
        );
        ap.refer(&mut image)
            .add_argument("image", Store, "Image to convert")
            .required();
        ap.parse_args_or_exit();
    }

    // Use either Nord or set color palette
    let colors: Vec<_> = if colors.is_empty() {
        NORD.to_vec()
    } else {
        colors.split(',').collect()
    };

    // Generate Color Palette based on
    println!("[1/4] Generating Color Space");
    let palette = ColorPaletteSpace::new(colors.as_slice())?;

    // Open Image
    println!("[2/4] Open Image");
    let img = ImageReader::open(image.clone()).map_err(TransferError::IoError)?;
    // Use format from Output file, input file or fallback to jpeg
    let format = ImageFormat::from_path(&output)
        .unwrap_or_else(|_| img.format().unwrap_or(ImageFormat::Jpeg));
    let img = img.decode().map_err(TransferError::ImgError)?.to_rgb8();
    let dim = img.dimensions();

    let counter = Arc::new(RelaxedCounter::new(0));
    let counter2 = counter.clone();
    let num_pixel = dim.0 * dim.1;
    println!("[3/4] Calculating Pixel");
    let pb = Arc::new(RwLock::new(ProgressBar::new(num_pixel as u64)));
    pb.write().unwrap().set_style(
        ProgressStyle::default_bar()
            .template("[{elapsed_precise}] {wide_bar} {pos:>7}/{len:7}")
            .progress_chars("##-"),
    );
    let local_pb = pb.clone();
    thread::spawn(move || loop {
        let num = counter2.get();
        pb.write().unwrap().set_position(num as u64);
        if num >= num_pixel as usize {
            pb.write().unwrap().finish_and_clear();
            return;
        }
        sleep(Duration::from_millis(12));
    });

    let mut pixel: Vec<_> = img.pixels().cloned().collect();
    // Apply new colors in parallel
    let bytes: Vec<u8> = pixel
        .par_drain(..)
        .flat_map_iter(|rgb| {
            let rgb = palette.get_color(&rgb.0);
            counter.inc();
            rgb
        })
        .collect();
    local_pb.write().unwrap().finish_and_clear();

    // Determine output name
    let output = if output.is_empty() {
        format!(
            "{}-out.{}",
            image.split('.').next().unwrap_or("o"),
            format.extensions_str()[0]
        )
    } else {
        output
    };
    // Write to file
    println!("[4/4] Write Image");
    save_buffer_with_format(output, &bytes, dim.0, dim.1, image::ColorType::Rgb8, format)
        .map_err(TransferError::ImgError)?;

    Ok(())
}
