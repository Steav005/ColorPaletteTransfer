use argparse::{ArgumentParser, Store, StoreTrue};
use chashmap::CHashMap;
use image::io::Reader as ImageReader;
use image::save_buffer_with_format;
use image::{ImageError, ImageFormat};
use ncollide3d::math::Point;
use ncollide3d::na::Isometry3;
use ncollide3d::query::{closest_points, ClosestPoints};
use ncollide3d::shape::{Ball, ConvexHull};
use palette::rgb::FromHexError;
use palette::Srgb;
use rayon::prelude::*;
use std::str::FromStr;
use std::time::Instant;

const NORD: [&str; 16] = [
    "#2E3440", "#3B4252", "#434C5E", "#4C566A", "#D8DEE9", "#E5E9F0", "#ECEFF4", "#8FBCBB",
    "#88C0D0", "#81A1C1", "#5E81AC", "#BF616A", "#D08770", "#EBCB8B", "#A3BE8C", "#B48EAD",
];

#[derive(Debug)]
enum TransferError {
    IoError(std::io::Error),
    ImgError(ImageError),
    HexError(FromHexError),
    ConvexHullError,
}

fn main() -> Result<(), TransferError> {
    let mut timing = false;
    let mut output = String::from("");
    let mut colors = String::from("");
    let mut image = String::from("");
    {
        let mut ap = ArgumentParser::new();
        ap.set_description("Converts image to color palette");
        ap.refer(&mut timing)
            .add_option(&["-t", "--timing"], StoreTrue, "Prints timings");
        ap.refer(&mut output)
            .add_option(&["-o", "--output"], Store, "Set output name. \
            Tries to honour set extension. \
            Example: output.png");
        ap.refer(&mut colors).add_option(
            &["-c", "--colors"],
            Store,
            "Hexcodes in parenthesis and split by comma. \
            Example: \"2E3440,3B4252,434C5E\". \
            Uses Nord color palette if not set: https://www.nordtheme.com/",
        );
        ap.refer(&mut image)
            .add_argument("image", Store, "Image to convert");
        ap.parse_args_or_exit();
    }

    // Use either Nord or set color palette
    let colors: Vec<_> = if colors.is_empty() {
        NORD.iter().cloned().collect()
    } else {
        colors.split(",").collect()
    };

    // Generate Color Palette based on
    let palette = ColorPaletteSpace::new(colors.as_slice())?;

    let now = Instant::now();
    // Open Image
    let img = ImageReader::open(image).map_err(|e| TransferError::IoError(e))?;
    // Use format from Output file, input file or fallback to jpeg
    let format = ImageFormat::from_path(&output)
        .unwrap_or_else(|_| img.format().unwrap_or_else(|| ImageFormat::Jpeg));
    let img = img
        .decode()
        .map_err(|e| TransferError::ImgError(e))?
        .to_rgb8();
    let dim = img.dimensions();
    if timing {
        println!("Read took {:?}", now.elapsed());
    }
    let now = Instant::now();

    let mut pixel: Vec<_> = img.pixels().cloned().collect();
    // Apply new colors in parallel
    let bytes: Vec<u8> = pixel
        .par_drain(..)
        .flat_map_iter(|rgb| palette.get_color(&rgb.0))
        .collect();
    if timing {
        println!("Transfer took {:?}", now.elapsed());
    }
    let now = Instant::now();

    // Determine output name
    let output = if output.is_empty() {
        format!("out.{}", format.extensions_str()[0])
    } else {
        output
    };
    // Write to file
    save_buffer_with_format(output, &bytes, dim.0, dim.1, image::ColorType::Rgb8, format)
        .map_err(|e| TransferError::ImgError(e))?;
    if timing {
        println!("Write took {:?}", now.elapsed());
    }

    Ok(())
}

struct ColorPaletteSpace {
    colorspace: ConvexHull<f64>,
    zero_ball: Ball<f64>,
    zero_iso: Isometry3<f64>,
    cache: CHashMap<[u8; 3], [u8; 3]>,
}

impl ColorPaletteSpace {
    fn new(palette: &[&str]) -> Result<ColorPaletteSpace, TransferError> {
        let mut color_points = Vec::new();
        // Generate Color Palette from Hex Codes
        for c in palette {
            let c = Srgb::from_str(c).map_err(|e| TransferError::HexError(e))?;
            color_points.push(Point::new(c.red as f64, c.green as f64, c.blue as f64))
        }
        // Build Convex Hull around color palette
        let colorspace = ConvexHull::try_from_points(&color_points)
            .ok_or_else(|| TransferError::ConvexHullError)?;

        // Initialise helper structs
        let zero_ball = Ball::new(0f64);
        let zero_iso = Isometry3::translation(0f64, 0f64, 0f64);
        let cache = CHashMap::new();

        Ok(ColorPaletteSpace {
            colorspace,
            zero_ball,
            zero_iso,
            cache,
        })
    }

    fn get_color(&self, rgb: &[u8; 3]) -> [u8; 3] {
        // Check if we calculated this rgb value before
        if let Some(rgb) = self.cache.get(rgb) {
            return *rgb;
        }
        // Use translation with zero radius sphere as our color point
        let point = Isometry3::translation(rgb[0] as f64, rgb[1] as f64, rgb[2] as f64);
        // Determine closest point of our color space hull to our color point
        let new = match closest_points(
            &self.zero_iso,
            &self.colorspace,
            &point,
            &self.zero_ball,
            99999f64,
        ) {
            ClosestPoints::Intersecting => *rgb,
            ClosestPoints::WithinMargin(new, _) => new.coords.data.0[0].map(|i| i as u8),
            ClosestPoints::Disjoint => panic!(),
        };

        // Insert new color into cache
        self.cache.insert(*rgb, new);
        new
    }
}
