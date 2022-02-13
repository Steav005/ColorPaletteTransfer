use chashmap::CHashMap;
use image::ImageError;
use palette::rgb::FromHexError;
use palette::Srgb;
use parry3d::math::Point;
use parry3d::na::Isometry3;
use parry3d::query::{closest_points, ClosestPoints};
use parry3d::shape::{Ball, ConvexPolyhedron};
use std::str::FromStr;

pub const NORD: [&str; 16] = [
    "#2E3440", "#3B4252", "#434C5E", "#4C566A", "#D8DEE9", "#E5E9F0", "#ECEFF4", "#8FBCBB",
    "#88C0D0", "#81A1C1", "#5E81AC", "#BF616A", "#D08770", "#EBCB8B", "#A3BE8C", "#B48EAD",
];

#[derive(Debug)]
pub enum TransferError {
    IoError(std::io::Error),
    ImgError(ImageError),
    HexError(FromHexError),
    ConvexHullError,
}

pub struct ColorPaletteSpace {
    colorspace: ConvexPolyhedron,
    zero_ball: Ball,
    zero_iso: Isometry3<f32>,
    cache: CHashMap<[u8; 3], [u8; 3]>,
}

impl ColorPaletteSpace {
    pub fn new(palette: &[&str]) -> Result<ColorPaletteSpace, TransferError> {
        let mut color_points = Vec::new();
        // Generate Color Palette from Hex Codes
        for c in palette {
            let c = Srgb::from_str(c).map_err(TransferError::HexError)?;
            color_points.push(Point::new(c.red as f32, c.green as f32, c.blue as f32))
        }
        // Build Convex Hull around color palette
        let colorspace = ConvexPolyhedron::from_convex_hull(&color_points)
            .ok_or(TransferError::ConvexHullError)?;

        // Initialise helper structs
        let zero_ball = Ball::new(0f32);
        let zero_iso = Isometry3::translation(0f32, 0f32, 0f32);
        let cache = CHashMap::new();

        Ok(ColorPaletteSpace {
            colorspace,
            zero_ball,
            zero_iso,
            cache,
        })
    }

    pub fn get_color(&self, rgb: &[u8; 3]) -> [u8; 3] {
        // Check if we calculated this rgb value before
        if let Some(rgb) = self.cache.get(rgb) {
            return *rgb;
        }
        // Use translation with zero radius sphere as our color point
        let point = Isometry3::translation(rgb[0] as f32, rgb[1] as f32, rgb[2] as f32);
        // Determine closest point of our color space hull to our color point
        let new = match closest_points(
            &self.zero_iso,
            &self.colorspace,
            &point,
            &self.zero_ball,
            99999f32,
        )
        .expect("Compatible Shapes")
        {
            ClosestPoints::Intersecting => *rgb,
            ClosestPoints::WithinMargin(new, _) => new.coords.data.0[0].map(|i| i as u8),
            ClosestPoints::Disjoint => panic!(),
        };

        // Insert new color into cache
        self.cache.insert(*rgb, new);
        new
    }
}
