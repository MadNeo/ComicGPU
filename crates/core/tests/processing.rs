use comicgpu_core::*;
use image::{Rgb, RgbImage};
use std::io::{Cursor, Read, Write};
use zip::{write::SimpleFileOptions, ZipArchive, ZipWriter};
fn fixture() -> RgbImage {
    RgbImage::from_fn(180, 240, |x, y| {
        if (20..160).contains(&x) && y % 30 < 3 {
            Rgb([20; 3])
        } else {
            Rgb([240, 230, 210])
        }
    })
}
fn archive(entries: Vec<(&str, Vec<u8>)>) -> Vec<u8> {
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, data) in entries {
        zip.start_file(name, SimpleFileOptions::default()).unwrap();
        zip.write_all(&data).unwrap();
    }
    zip.finish().unwrap().into_inner()
}
#[test]
fn digital_preserves_pixels() {
    let image = fixture();
    assert_eq!(restore(image.clone(), &Options::default()).unwrap(), image);
}
#[test]
fn filters_and_scaling() {
    let options = Options {
        paper: 0.4,
        shadow: 0.2,
        bleed: 0.2,
        moire: 0.3,
        grayscale: true,
        upscale: true,
        ..Default::default()
    };
    let result = restore(fixture(), &options).unwrap();
    assert_eq!(result.dimensions(), (360, 480));
    assert!(result.pixels().all(|p| p[0] == p[1] && p[1] == p[2]));
}
#[test]
fn invalid_strength_rejected() {
    assert!(restore(
        fixture(),
        &Options {
            paper: 1.5,
            ..Default::default()
        }
    )
    .is_err());
}
#[test]
fn natural_sort_corruption_and_duplicate_detection() {
    let bytes = png(fixture()).unwrap();
    let data = archive(vec![
        ("10.png", bytes.clone()),
        ("2.png", bytes),
        ("3.png", b"broken".to_vec()),
    ]);
    let book = read_book(&data, "test.cbz").unwrap();
    assert_eq!(book.pages[0].info.name, "2.png");
    assert_eq!(book.warnings.len(), 1);
    assert_eq!(book.pages[1].source_index, 2);
    assert!(book.pages[1]
        .info
        .review
        .iter()
        .any(|s| s.starts_with("Mogelijk dubbel")));
}
#[test]
fn manga_split_and_metadata_remap_after_corruption() {
    let spread = RgbImage::from_fn(
        301,
        100,
        |x, _| if x < 150 { Rgb([255; 3]) } else { Rgb([0; 3]) },
    );
    let metadata=b"<ComicInfo><Title>Test</Title><Pages><Page Image=\"1\" Type=\"FrontCover\"/></Pages></ComicInfo>";
    let data = archive(vec![
        ("1.png", b"broken".to_vec()),
        ("2.png", png(spread).unwrap()),
        ("ComicInfo.xml", metadata.to_vec()),
    ]);
    let book = read_book(&data, "test.cbz").unwrap();
    let options = vec![Options {
        split: true,
        rtl: true,
        approved: true,
        ..Default::default()
    }];
    let result = export_book(&book, &options, 2, false).unwrap();
    let mut zip = ZipArchive::new(Cursor::new(result)).unwrap();
    assert_eq!(zip.len(), 3);
    let mut bytes = Vec::new();
    zip.by_name("0001.png")
        .unwrap()
        .read_to_end(&mut bytes)
        .unwrap();
    let image = decode(&bytes).unwrap();
    assert_eq!(image.width(), 151);
    assert_eq!(image.get_pixel(0, 0), &Rgb([0; 3]));
    let xml = xmltree::Element::parse(zip.by_name("ComicInfo.xml").unwrap()).unwrap();
    assert_eq!(xml.get_child("Title").unwrap().get_text().unwrap(), "Test");
    assert_eq!(xml.get_child("PageCount").unwrap().get_text().unwrap(), "2");
    assert_eq!(xml.get_child("Pages").unwrap().children.len(), 2);
}
#[test]
fn review_gate_and_empty_export() {
    let book = read_book(
        &archive(vec![("1.png", png(fixture()).unwrap())]),
        "test.cbz",
    )
    .unwrap();
    assert!(export_book(&book, &[Options::default()], 1, false).is_err());
    assert!(export_book(
        &book,
        &[Options {
            excluded: true,
            ..Default::default()
        }],
        1,
        true
    )
    .is_err());
    assert!(export_book(&book, &[], 1, true).is_err());
}
#[test]
fn crop_retains_content_margin() {
    let image = RgbImage::from_fn(800, 1000, |x, y| {
        if (30..770).contains(&x) && (30..970).contains(&y) {
            Rgb([40; 3])
        } else {
            Rgb([255; 3])
        }
    });
    let result = restore(
        image,
        &Options {
            crop: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(result.dimensions(), (756, 956));
    assert_eq!(result.get_pixel(8, 8), &Rgb([40; 3]));
}
#[test]
fn gpu_shader_executes_when_adapter_available() {
    if gpu::description().is_none() {
        eprintln!("GPU-test overgeslagen: geen adapter");
        return;
    }
    let image = RgbImage::from_pixel(16, 16, Rgb([90, 120, 200]));
    let result = gpu::upscale(&image).unwrap();
    assert_eq!(result.dimensions(), (32, 32));
    assert!(result.pixels().all(|p| p
        .0
        .iter()
        .zip([90_u8, 120, 200])
        .all(|(a, b)| a.abs_diff(b) <= 1)));
}

#[test]
fn cbr_reads_embedded_rar_without_external_programs() {
    fn crc(bytes: &[u8]) -> u32 {
        let mut value = !0_u32;
        for byte in bytes {
            value ^= u32::from(*byte);
            for _ in 0..8 {
                value = (value >> 1) ^ (0xedb88320_u32 & 0_u32.wrapping_sub(value & 1));
            }
        }
        !value
    }
    fn header(kind: u8, flags: u16, body: &[u8]) -> Vec<u8> {
        let mut bytes = vec![kind];
        bytes.extend(flags.to_le_bytes());
        bytes.extend(((body.len() + 7) as u16).to_le_bytes());
        bytes.extend(body);
        let mut result = (crc(&bytes) as u16).to_le_bytes().to_vec();
        result.extend(bytes);
        result
    }
    let image = png(fixture()).unwrap();
    let mut data = b"Rar!\x1a\x07\x00".to_vec();
    data.extend(header(0x73, 0, &[0; 6]));
    let mut body = Vec::new();
    body.extend((image.len() as u32).to_le_bytes());
    body.extend((image.len() as u32).to_le_bytes());
    body.push(3);
    body.extend(crc(&image).to_le_bytes());
    body.extend(0_u32.to_le_bytes());
    body.extend([20, 0x30]);
    body.extend(5_u16.to_le_bytes());
    body.extend(0o100644_u32.to_le_bytes());
    body.extend(b"1.png");
    data.extend(header(0x74, 0x8000, &body));
    data.extend(image);
    data.extend(header(0x7b, 0, &[]));
    let book = read_book(&data, "test.cbr").unwrap();
    assert_eq!(book.pages.len(), 1);
    assert_eq!(book.pages[0].info.name, "1.png");
    let zip = archive(vec![("1.png", png(fixture()).unwrap())]);
    assert_eq!(read_book(&zip, "misnamed.cbr").unwrap().pages.len(), 1);
}
