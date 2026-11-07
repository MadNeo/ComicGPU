use crate::{decode, fingerprint, png, restore, split, Options};
use anyhow::{ensure, Context, Result};
use rayon::prelude::*;
use serde::Serialize;
use std::{
    io::{Cursor, Read, Write},
    path::Path,
    process::{Command, Stdio},
};
use xmltree::{Element, XMLNode};
use zip::{write::SimpleFileOptions, ZipArchive, ZipWriter};

pub const MAX_BYTES: u64 = 512 * 1024 * 1024;
const MAX_PAGE_BYTES: u64 = 64 * 1024 * 1024;
#[derive(Clone, Serialize)]
pub struct PageInfo {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub review: Vec<String>,
}
pub struct Page {
    pub info: PageInfo,
    pub data: Vec<u8>,
    pub source_index: usize,
}
pub struct Book {
    pub pages: Vec<Page>,
    pub metadata: Option<Vec<u8>>,
    pub warnings: Vec<String>,
}
fn image_name(name: &str) -> bool {
    matches!(
        Path::new(name)
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_ascii_lowercase()
            .as_str(),
        "jpg" | "jpeg" | "png" | "webp" | "bmp" | "tif" | "tiff"
    )
}
fn bounded_read(reader: impl Read, maximum: u64) -> Result<Vec<u8>> {
    let mut data = Vec::new();
    reader.take(maximum + 1).read_to_end(&mut data)?;
    ensure!(
        data.len() as u64 <= maximum,
        "Bestand overschrijdt de toegestane grootte"
    );
    Ok(data)
}
// Stream selected RAR members to stdout. No archive paths are written to disk.
fn rar_command(path: &Path, member: Option<&str>, maximum: u64) -> Result<Vec<u8>> {
    let program = if cfg!(windows) {
        let system = std::env::var_os("SystemRoot").unwrap_or_else(|| r"C:\Windows".into());
        std::path::PathBuf::from(system)
            .join("System32")
            .join("tar.exe")
    } else {
        std::path::PathBuf::from("bsdtar")
    };
    let mut command = Command::new(program);
    command
        .arg(if member.is_some() { "-xOf" } else { "-tf" })
        .arg(path);
    if let Some(name) = member {
        command.arg("--").arg(name);
    }
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .context(
            "CBR vereist libarchive-tools op Linux of Windows tar.exe met RAR-ondersteuning",
        )?;
    let result = bounded_read(child.stdout.take().context("Uitvoer ontbreekt")?, maximum);
    if result.is_err() {
        let _ = child.kill();
    }
    let status = child.wait()?;
    let bytes = result?;
    ensure!(status.success(), "CBR-bestand kon niet worden uitgepakt");
    Ok(bytes)
}
pub fn read_book(data: &[u8], filename: &str) -> Result<Book> {
    ensure!(data.len() as u64 <= MAX_BYTES, "Archief groter dan 512 MB");
    let mut members = Vec::<(String, Vec<u8>)>::new();
    if filename.to_ascii_lowercase().ends_with(".cbr") {
        let mut file = tempfile::NamedTempFile::new()?;
        file.write_all(data)?;
        let listing = String::from_utf8(rar_command(file.path(), None, 1024 * 1024)?)?;
        ensure!(
            listing.lines().count() <= 3000,
            "Meer dan 3000 archiefitems"
        );
        let mut total = 0_u64;
        for name in listing.lines() {
            if image_name(name) || name.to_ascii_lowercase().ends_with("comicinfo.xml") {
                ensure!(
                    !name.contains(['*', '?', '[']),
                    "CBR-bestandsnaam bevat niet-ondersteunde jokertekens"
                );
                let bytes = rar_command(file.path(), Some(name), MAX_PAGE_BYTES)?;
                total += bytes.len() as u64;
                ensure!(total <= MAX_BYTES, "Uitgepakte inhoud groter dan 512 MB");
                members.push((name.to_string(), bytes));
            }
        }
    } else {
        let mut archive = ZipArchive::new(Cursor::new(data))?;
        ensure!(archive.len() <= 3000, "Meer dan 3000 archiefitems");
        let mut total = 0_u64;
        for index in 0..archive.len() {
            let entry = archive.by_index(index)?;
            total = total
                .checked_add(entry.size())
                .context("Archiefgrootte ongeldig")?;
            ensure!(total <= MAX_BYTES, "Uitgepakte inhoud groter dan 512 MB");
            let name = entry.name().to_string();
            if image_name(&name) || name.to_ascii_lowercase().ends_with("comicinfo.xml") {
                members.push((name, bounded_read(entry, MAX_PAGE_BYTES)?));
            }
        }
    }
    members.sort_by(|a, b| natord::compare(&a.0, &b.0));
    let mut book = Book {
        pages: Vec::new(),
        metadata: None,
        warnings: Vec::new(),
    };
    let mut hashes: Vec<u64> = Vec::new();
    let mut source_index = 0;
    for (name, bytes) in members {
        if name.to_ascii_lowercase().ends_with("comicinfo.xml") {
            book.metadata = Some(bytes);
            continue;
        }
        match decode(&bytes) {
            Ok(image) => {
                let hash = fingerprint(&image);
                let mut review = Vec::new();
                if let Some(index) = hashes
                    .iter()
                    .position(|old| (*old ^ hash).count_ones() <= 3)
                {
                    review.push(format!(
                        "Mogelijk dubbel met {}",
                        book.pages[index].info.name
                    ));
                }
                if image.width() as f32 > image.height() as f32 * 1.25 {
                    review.push("Mogelijke dubbele pagina".into());
                }
                if image.width().min(image.height()) < 700 {
                    review.push("Lage resolutie".into());
                }
                hashes.push(hash);
                book.pages.push(Page {
                    info: PageInfo {
                        name,
                        width: image.width(),
                        height: image.height(),
                        review,
                    },
                    data: bytes,
                    source_index,
                });
            }
            Err(error) => book.warnings.push(format!("{name}: onleesbaar ({error})")),
        }
        source_index += 1;
    }
    ensure!(!book.pages.is_empty(), "Geen leesbare pagina’s");
    Ok(book)
}
fn update_metadata(bytes: &[u8], mapping: &[(usize, Vec<usize>)], total: usize) -> Result<Vec<u8>> {
    let text = String::from_utf8_lossy(bytes).to_ascii_uppercase();
    ensure!(
        !text.contains("<!DOCTYPE") && !text.contains("<!ENTITY"),
        "ComicInfo bevat niet-ondersteunde XML-declaraties"
    );
    let mut root = Element::parse(bytes)?;
    if root.get_child("PageCount").is_none() {
        root.children
            .push(XMLNode::Element(Element::new("PageCount")));
    }
    root.get_mut_child("PageCount").unwrap().children = vec![XMLNode::Text(total.to_string())];
    if let Some(pages) = root.get_mut_child("Pages") {
        let mut replacement = Vec::new();
        for node in &pages.children {
            if let XMLNode::Element(page) = node {
                let old = page
                    .attributes
                    .get("Image")
                    .and_then(|s| s.parse::<usize>().ok());
                if let Some((_, indices)) = mapping.iter().find(|(index, _)| Some(*index) == old) {
                    for index in indices {
                        let mut page = page.clone();
                        page.attributes.insert("Image".into(), index.to_string());
                        for name in ["ImageSize", "ImageWidth", "ImageHeight", "DoublePage"] {
                            page.attributes.remove(name);
                        }
                        replacement.push(XMLNode::Element(page));
                    }
                }
            }
        }
        pages.children = replacement;
    }
    let mut output = Vec::new();
    root.write(&mut output)?;
    Ok(output)
}
pub fn export_book(
    book: &Book,
    options: &[Options],
    workers: usize,
    allow_unreviewed: bool,
) -> Result<Vec<u8>> {
    ensure!(
        options.len() == book.pages.len(),
        "Instellingen horen niet bij dit aantal pagina’s"
    );
    ensure!(
        (1..=4).contains(&workers),
        "Kies 1 tot 4 parallelle pagina’s"
    );
    for (page, option) in book.pages.iter().zip(options) {
        option.validate()?;
        ensure!(
            allow_unreviewed || page.info.review.is_empty() || option.approved || option.excluded,
            "Beoordeel eerst de gemarkeerde pagina’s"
        );
    }
    let selected: Vec<_> = book
        .pages
        .iter()
        .zip(options)
        .filter(|(_, o)| !o.excluded)
        .collect();
    ensure!(!selected.is_empty(), "Selecteer minimaal één pagina");
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(workers)
        .build()?;
    let mut archive = ZipWriter::new(Cursor::new(Vec::new()));
    let mut counter = 0;
    let mut mapping = Vec::new();
    // Only one worker-sized chunk of decoded/output pages exists at a time.
    for chunk in selected.chunks(workers) {
        let outputs: Result<Vec<_>> = pool.install(|| {
            chunk
                .par_iter()
                .map(|(page, option)| {
                    let images = split(restore(decode(&page.data)?, option)?, option);
                    let encoded: Result<Vec<_>> = images.into_iter().map(png).collect();
                    Ok((page.source_index, encoded?))
                })
                .collect()
        });
        for (source, images) in outputs? {
            let mut indices = Vec::new();
            for bytes in images {
                archive.start_file(
                    format!("{:04}.png", counter + 1),
                    SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored),
                )?;
                archive.write_all(&bytes)?;
                indices.push(counter);
                counter += 1;
            }
            mapping.push((source, indices));
        }
    }
    if let Some(metadata) = &book.metadata {
        archive.start_file("ComicInfo.xml", SimpleFileOptions::default())?;
        archive.write_all(&update_metadata(metadata, &mapping, counter)?)?;
    }
    Ok(archive.finish()?.into_inner())
}
