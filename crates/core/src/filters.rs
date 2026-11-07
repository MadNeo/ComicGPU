use crate::Options;
use anyhow::{ensure, Result};
use image::{imageops, DynamicImage, Rgb, RgbImage};
use imageproc::{
    filter::median_filter,
    geometric_transformations::{rotate_about_center, Interpolation},
};
use std::io::Cursor;

pub fn decode(data: &[u8]) -> Result<RgbImage> {
    let mut reader = image::ImageReader::new(Cursor::new(data)).with_guessed_format()?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(16000);
    limits.max_image_height = Some(16000);
    limits.max_alloc = Some(256 * 1024 * 1024);
    reader.limits(limits);
    let dimensions = image::ImageReader::new(Cursor::new(data))
        .with_guessed_format()?
        .into_dimensions()?;
    ensure!(
        u64::from(dimensions.0) * u64::from(dimensions.1) <= 32_000_000,
        "Pagina groter dan 32 megapixels"
    );
    let mut decoder = reader.into_decoder()?;
    use image::ImageDecoder;
    let orientation = decoder.orientation()?;
    let mut decoded = DynamicImage::from_decoder(decoder)?;
    decoded.apply_orientation(orientation);
    let rgba = decoded.to_rgba8();
    // Composite transparent pixels onto paper rather than turning them black.
    Ok(RgbImage::from_fn(rgba.width(), rgba.height(), |x, y| {
        let p = rgba.get_pixel(x, y);
        Rgb(std::array::from_fn(|c| {
            ((u16::from(p[c]) * u16::from(p[3]) + 255 * (255 - u16::from(p[3]))) / 255) as u8
        }))
    }))
}
pub fn png(image: RgbImage) -> Result<Vec<u8>> {
    let mut data = Cursor::new(Vec::new());
    DynamicImage::ImageRgb8(image).write_to(&mut data, image::ImageFormat::Png)?;
    Ok(data.into_inner())
}
fn blend(original: f32, corrected: f32, strength: f32) -> u8 {
    (original + (corrected - original) * strength)
        .round()
        .clamp(0., 255.) as u8
}
fn score(image: &RgbImage) -> f64 {
    let rows: Vec<f64> = image
        .rows()
        .map(|row| {
            row.filter(|p| p[0].min(p[1]).min(p[2]) < 100).count() as f64 / image.width() as f64
        })
        .collect();
    let average = rows.iter().sum::<f64>() / rows.len() as f64;
    rows.iter().map(|v| (v - average).powi(2)).sum::<f64>() / rows.len() as f64
}
fn deskew(image: RgbImage) -> RgbImage {
    let small = imageops::thumbnail(&image, 600, 600);
    let baseline = score(&small);
    let mut best = (baseline, 0.0_f32);
    for step in -12..=12 {
        let angle = (step as f32 * 0.25).to_radians();
        let rotated = rotate_about_center(&small, angle, Interpolation::Nearest, Rgb([255; 3]));
        let candidate = score(&rotated);
        if candidate > best.0 {
            best = (candidate, angle);
        }
    }
    if best.0 <= baseline * 1.08 || best.1.abs() >= 3.0_f32.to_radians() {
        return image;
    }
    let (sin, cos) = best.1.sin_cos();
    let width =
        (image.width() as f32 * cos.abs() + image.height() as f32 * sin.abs()).ceil() as u32;
    let height =
        (image.height() as f32 * cos.abs() + image.width() as f32 * sin.abs()).ceil() as u32;
    let mut canvas = RgbImage::from_pixel(width, height, Rgb([255; 3]));
    imageops::replace(
        &mut canvas,
        &image,
        ((width - image.width()) / 2) as i64,
        ((height - image.height()) / 2) as i64,
    );
    rotate_about_center(&canvas, best.1, Interpolation::Bicubic, Rgb([255; 3]))
}
fn crop(image: RgbImage) -> RgbImage {
    let (w, h) = image.dimensions();
    let uniform = |pixels: Vec<&Rgb<u8>>| {
        let total = pixels.len() as f32;
        let white = pixels
            .iter()
            .filter(|p| p.0.iter().all(|v| *v > 245))
            .count() as f32;
        let black = pixels
            .iter()
            .filter(|p| p.0.iter().all(|v| *v < 12))
            .count() as f32;
        white / total > 0.995 || black / total > 0.995
    };
    let left = (0..w / 20)
        .take_while(|x| uniform((0..h).map(|y| image.get_pixel(*x, y)).collect()))
        .count()
        .saturating_sub(8) as u32;
    let right = (0..w / 20)
        .take_while(|x| uniform((0..h).map(|y| image.get_pixel(w - 1 - *x, y)).collect()))
        .count()
        .saturating_sub(8) as u32;
    let top = (0..h / 20)
        .take_while(|y| uniform((0..w).map(|x| image.get_pixel(x, *y)).collect()))
        .count()
        .saturating_sub(8) as u32;
    let bottom = (0..h / 20)
        .take_while(|y| uniform((0..w).map(|x| image.get_pixel(x, h - 1 - *y)).collect()))
        .count()
        .saturating_sub(8) as u32;
    imageops::crop_imm(&image, left, top, w - left - right, h - top - bottom).to_image()
}
pub fn restore(mut image: RgbImage, options: &Options) -> Result<RgbImage> {
    options.validate()?;
    if options.deskew {
        image = deskew(image);
    }
    if options.crop {
        image = crop(image);
    }
    if options.paper > 0. {
        let mut histogram = [[0_u64; 256]; 3];
        for pixel in image.pixels() {
            for c in 0..3 {
                histogram[c][pixel[c] as usize] += 1;
            }
        }
        let threshold = (image.width() as u64 * image.height() as u64 * 95) / 100;
        let white: [f32; 3] = std::array::from_fn(|c| {
            let mut count = 0;
            histogram[c]
                .iter()
                .position(|n| {
                    count += n;
                    count >= threshold
                })
                .unwrap_or(255)
                .max(180) as f32
        });
        for pixel in image.pixels_mut() {
            for c in 0..3 {
                pixel[c] = blend(
                    pixel[c] as f32,
                    pixel[c] as f32 * 255. / white[c],
                    options.paper,
                );
            }
        }
    }
    if options.shadow > 0. {
        let small = imageops::resize(&image, 64, 64, imageops::FilterType::Triangle);
        let background = imageops::resize(
            &imageops::blur(&small, 8.),
            image.width(),
            image.height(),
            imageops::FilterType::Triangle,
        );
        for (pixel, light) in image.pixels_mut().zip(background.pixels()) {
            for c in 0..3 {
                pixel[c] = blend(
                    pixel[c] as f32,
                    pixel[c] as f32 * 230. / (light[c] as f32).max(120.),
                    options.shadow,
                );
            }
        }
    }
    if options.bleed > 0. {
        for pixel in image.pixels_mut() {
            if pixel.0.iter().all(|v| *v > 170) {
                for c in 0..3 {
                    pixel[c] = blend(pixel[c] as f32, 255., options.bleed);
                }
            }
        }
    }
    if options.moire > 0. {
        let smooth = median_filter(&image, 1, 1);
        for (pixel, filtered) in image.pixels_mut().zip(smooth.pixels()) {
            for c in 0..3 {
                pixel[c] = blend(pixel[c] as f32, filtered[c] as f32, options.moire);
            }
        }
    }
    if options.grayscale {
        image = DynamicImage::ImageLuma8(imageops::grayscale(&image)).to_rgb8();
    }
    if options.upscale {
        ensure!(
            image.width() as u64 * image.height() as u64 <= 16_000_000,
            "2× opschalen ondersteunt maximaal 16 megapixels invoer"
        );
        image = if options.gpu {
            crate::gpu::upscale(&image)?
        } else {
            imageops::resize(
                &image,
                image.width() * 2,
                image.height() * 2,
                imageops::FilterType::Lanczos3,
            )
        };
    }
    Ok(image)
}
pub fn split(image: RgbImage, options: &Options) -> Vec<RgbImage> {
    if options.split && image.width() as f32 > image.height() as f32 * 1.25 {
        let center = image.width() / 2;
        let mut parts = vec![
            imageops::crop_imm(&image, 0, 0, center, image.height()).to_image(),
            imageops::crop_imm(&image, center, 0, image.width() - center, image.height())
                .to_image(),
        ];
        if options.rtl {
            parts.reverse();
        }
        parts
    } else {
        vec![image]
    }
}
pub fn fingerprint(image: &RgbImage) -> u64 {
    let small = imageops::resize(
        &imageops::grayscale(image),
        9,
        8,
        imageops::FilterType::Triangle,
    );
    let mut hash = 0_u64;
    for y in 0..8 {
        for x in 0..8 {
            hash = (hash << 1) | u64::from(small.get_pixel(x + 1, y)[0] > small.get_pixel(x, y)[0]);
        }
    }
    hash
}
