use exif::{In, Reader, Tag};
use image::{imageops::FilterType, DynamicImage, GenericImageView, ImageFormat, Pixel, Rgb};
use std::{collections::HashMap, fmt::Write, io::Cursor};
use wasm_bindgen::prelude::*;
use serde_wasm_bindgen::to_value;
use serde::Serialize;

#[derive(Serialize)]
struct ImageAnalysis {
    exif_data: Vec<[String; 2]>,
    unique_colors: Vec<String>,
}

/// Reads the orientation from the image EXIF data.
/// Returns the orientation as a `u16`. Defaults to `1` if not found.
fn read_orientation(image_data: &[u8]) -> u16 {
    // Wrap the byte slice in a Cursor, which implements BufRead and Seek
    let cursor: Cursor<&[u8]> = Cursor::new(image_data);

    // Create a new Reader without needing to pass any arguments
    let reader: Reader = Reader::new();

    // Attempt to read the EXIF data from the image
    match reader.read_from_container(&mut cursor.clone()) {
        Ok(exif) => {
            // Attempt to find the orientation tag in the primary IFD
            if let Some(field) = exif.get_field(Tag::Orientation, In::PRIMARY) {
                // If found, return its value as a `u16`
                match field.value.get_uint(0) {
                    Some(val) => val as u16,
                    None => 1, // Default orientation if the tag value is not readable
                }
            } else {
                // Default orientation if the tag is not found
                1
            }
        }
        Err(_) => {
            // Default orientation if EXIF data cannot be read
            1
        }
    }
}

/// Applies the appropriate transformation to the image based on its EXIF orientation.
fn apply_orientation(mut img: DynamicImage, orientation: u16) -> DynamicImage {
    match orientation {
        1 => img,             // Normal, no action needed
        2 => img.fliph(),     // Flipped horizontally
        3 => img.rotate180(), // Rotated 180 degrees
        4 => img.flipv(),     // Flipped vertically
        5 => {
            // Transposed: flipped horizontally then rotated 90 degrees CCW
            img = img.fliph();
            img.rotate270()
        }
        6 => img.rotate90(), // Rotated 90 degrees CW
        7 => {
            // Transverse: flipped horizontally then rotated 90 degrees CW
            img = img.fliph();
            img.rotate90()
        }
        8 => img.rotate270(), // Rotated 90 degrees CCW
        _ => img,             // Default case, no transformation
    }
}

// A simple function to convert RGB to HSB
fn rgb_to_hsb(rgb: Rgb<u8>) -> (f32, f32, f32) {
    let r = rgb[0] as f32 / 255.0;
    let g = rgb[1] as f32 / 255.0;
    let b = rgb[2] as f32 / 255.0;

    let max = r.max(g.max(b));
    let min = r.min(g.min(b));
    let delta = max - min;

    let hue = if delta == 0.0 {
        0.0
    } else if max == r {
        60.0 * (((g - b) / delta) % 6.0)
    } else if max == g {
        60.0 * (((b - r) / delta) + 2.0)
    } else {
        60.0 * (((r - g) / delta) + 4.0)
    };

    let saturation = if max == 0.0 { 0.0 } else { delta / max };

    (hue, saturation, max)
}

// Function to calculate difference in hue, saturation and brightness
fn hsb_diff(hsb1: (f32, f32, f32), hsb2: (f32, f32, f32)) -> (f32, f32, f32) {
    let hue_diff = (hsb1.0 - hsb2.0).abs();
    let saturation_diff = (hsb1.1 - hsb2.1).abs();
    let brightness_diff = (hsb1.2 - hsb2.2).abs();
    (saturation_diff, brightness_diff, hue_diff)
}

fn get_unique_colors(image_data: &[u8]) -> Vec<String> {
    let img: DynamicImage = image::load_from_memory(image_data).expect("Failed to load image");
    let mut color_count: HashMap<[u8; 3], u32> = HashMap::new();

    for (_, _, pixel) in img.pixels() {
        let rgb = pixel.to_rgb().0;
        *color_count.entry(rgb).or_insert(0) += 1;
    }

    let all_colors = color_count.keys().collect::<Vec<_>>();

    let mut unique_colors = Vec::new();

    for &color in all_colors {
        let color_hsb: (f32, f32, f32) = rgb_to_hsb(Rgb(color)); // Corrected this line
        if unique_colors.iter().all(|&unique| {
            let (sat_diff, bri_diff, hue_diff) = hsb_diff(color_hsb, rgb_to_hsb(Rgb(unique)));
            sat_diff > 0.1 && bri_diff > 0.1 && hue_diff > 10.0 // Adjust thresholds as needed
        }) {
            unique_colors.push(color);
            if unique_colors.len() >= 20 {
                break;
            } // Limit to 5 unique colors
        }
    }

    let mut results = Vec::new();

    // Convert channel data to hex
    for color in unique_colors {
        let mut hex_color = String::new();
        write!(
            &mut hex_color,
            "#{:02X}{:02X}{:02X}",
            color[0], color[1], color[2]
        )
        .unwrap();
        results.push(hex_color);
    }

    results
}

fn parse_exif_data(image_data: &[u8]) -> Vec<[String; 2]> {
    // Initialize an empty vector to hold our EXIF tags as strings
    let mut exif_tags: Vec<[String; 2]> = Vec::new();
    // Create a cursor around the image data
    let cursor: Cursor<&[u8]> = Cursor::new(image_data);

    // Attempt to read the EXIF data using the exif crate
    match Reader::new().read_from_container(&mut cursor.clone()) {
        Ok(exif) => {
            for field in exif.fields() {
                // Create an array for each EXIF field with the tag name and its display value
                let tag_name: String = field.tag.to_string();
                let tag_value: String = field.display_value().with_unit(&exif).to_string();
                let tag_pair: [String; 2] = [tag_name, tag_value];
                
                // Push the array into our vector
                exif_tags.push(tag_pair);
            }
        },
        Err(e) => {
            // If there's an error reading the EXIF data, push the error message to the tags vector
            exif_tags.push(["Failed to read EXIF data".to_string(), e.to_string()]);
        }
    }
    // Convert our vector of strings to a JsValue to pass back to JavaScript
    // to_value(&exif_tags).unwrap_or(JsValue::NULL)
    exif_tags
}

#[wasm_bindgen]
pub fn analyze_image(image_data: &[u8]) -> JsValue {
    let exif_data: Vec<[String; 2]> = parse_exif_data(image_data);
    let unique_colors: Vec<String> = get_unique_colors(image_data);

    let analysis: ImageAnalysis = ImageAnalysis {
        exif_data,
        unique_colors,
    };

    // Convert the combined data into a JsValue
    to_value(&analysis).unwrap_or(JsValue::UNDEFINED)
}

#[wasm_bindgen]
pub fn resize_image(
    image_data: &[u8],
    width: u32,
    height: u32,
    format: &str,
    filter: &str,
) -> Vec<u8> {
    let img: DynamicImage = image::load_from_memory(image_data).unwrap();

    let orientation: u16 = read_orientation(image_data);
    let mut img: DynamicImage = apply_orientation(img, orientation);

    // Ensure the image is in a color space compatible with the target format.
    if format == "jpeg" {
        img = DynamicImage::ImageRgb8(img.to_rgb8());
    }

    let filter_type: FilterType = match filter {
        "catmull_rom" => FilterType::CatmullRom,
        "gaussian" => FilterType::Gaussian,
        "lanczos3" => FilterType::Lanczos3,
        "nearest" => FilterType::Nearest,
        "triangle" => FilterType::Triangle,
        _ => FilterType::Triangle, // Default filter
    };

    let resized: DynamicImage = img.resize_to_fill(width, height, filter_type);

    let image_format: ImageFormat = match format {
        "png" => ImageFormat::Png,
        "webp" => ImageFormat::WebP,
        "jpeg" => ImageFormat::Jpeg,
        "avif" => ImageFormat::Avif,
        "bmp" => ImageFormat::Bmp,
        "gif" => ImageFormat::Gif,
        "tiff" => ImageFormat::Tiff,
        "ico" => ImageFormat::Ico,
        _ => ImageFormat::Png, // Default format
    };

    let mut result: Vec<u8> = Vec::new();
    {
        let mut cursor: Cursor<&mut Vec<u8>> = Cursor::new(&mut result);
        resized.write_to(&mut cursor, image_format).unwrap();
    }

    result
}
