use chrono::{DateTime, Utc};
use ::image::GenericImageView;
use log::{error, warn};
use printpdf::*;
use pulldown_cmark::{Event, Parser, Tag, TagEnd};
use std::fs::File;
use std::io::BufWriter;

// ─── Page format ────────────────────────────────────────────────────────────

pub struct PageFormat {
    pub width_mm: f64,
    pub height_mm: f64,
}

pub fn parse_page_format(spec: &str) -> std::result::Result<PageFormat, String> {
    let lower = spec.to_lowercase();
    let (base, landscape) = if lower.ends_with("-landscape") {
        (&lower[..lower.len() - 10], true)
    } else if lower.ends_with("-portrait") {
        (&lower[..lower.len() - 9], false)
    } else {
        (lower.as_str(), true) // default to landscape
    };

    let (w, h) = match base {
        "a4" => (210.0_f64, 297.0_f64),
        "letter" => (215.9_f64, 279.4_f64),
        other => return Err(format!("Unknown page format '{}'. Use a4 or letter.", other)),
    };

    Ok(if landscape {
        PageFormat {
            width_mm: h,
            height_mm: w,
        }
    } else {
        PageFormat {
            width_mm: w,
            height_mm: h,
        }
    })
}

// ─── Layout mode ────────────────────────────────────────────────────────────

pub enum LayoutMode {
    Auto { threshold: chrono::Duration },
    Fixed(u8),
}

impl LayoutMode {
    pub fn from_args(
        images_per_page: &str,
        auto_group_minutes: u64,
    ) -> std::result::Result<Self, String> {
        match images_per_page {
            "auto" => Ok(LayoutMode::Auto {
                threshold: chrono::Duration::minutes(auto_group_minutes as i64),
            }),
            n => {
                let count: u8 = n.parse().map_err(|_| {
                    format!("Invalid --images-per-page '{}'. Use 1-4 or 'auto'.", n)
                })?;
                if !(1..=4).contains(&count) {
                    return Err("--images-per-page must be 1-4 or 'auto'".into());
                }
                Ok(LayoutMode::Fixed(count))
            }
        }
    }
}

// ─── Catalog entry / page group ─────────────────────────────────────────────

#[derive(Clone)]
pub struct CatalogEntry {
    pub url: String,
    pub caption: String,
    pub taken_at: Option<DateTime<Utc>>,
}

pub struct PageGroup {
    pub entries: Vec<CatalogEntry>,
}

/// Group catalog entries into pages using consecutive-pair temporal proximity.
pub fn auto_group_pages(entries: Vec<CatalogEntry>, threshold: chrono::Duration) -> Vec<PageGroup> {
    let mut pages: Vec<PageGroup> = Vec::new();
    let mut current: Vec<CatalogEntry> = Vec::new();

    for entry in entries {
        if current.is_empty() {
            current.push(entry);
            continue;
        }

        let should_extend = match (&current.last().unwrap().taken_at, &entry.taken_at) {
            (Some(prev), Some(curr)) => {
                let diff = *curr - *prev;
                diff >= chrono::Duration::zero() && diff <= threshold
            }
            _ => false,
        };

        if should_extend && current.len() < 4 {
            current.push(entry);
        } else {
            pages.push(PageGroup {
                entries: std::mem::take(&mut current),
            });
            current.push(entry);
        }
    }

    if !current.is_empty() {
        pages.push(PageGroup { entries: current });
    }

    pages
}

// ─── Grid layout ────────────────────────────────────────────────────────────

/// Returns (x, y, max_width, max_height) in mm for each cell, bottom-left origin.
/// printpdf uses a coordinate system where (0,0) is the bottom-left of the page.
pub fn calculate_grid_cells(
    page_width_mm: f64,
    page_height_mm: f64,
    margin_mm: f64,
    images_per_page: u8,
) -> Vec<(f64, f64, f64, f64)> {
    let usable_w = page_width_mm - 2.0 * margin_mm;
    let usable_h = page_height_mm - 2.0 * margin_mm;
    let gap = 5.0_f64;

    match images_per_page {
        1 => vec![(margin_mm, margin_mm, usable_w, usable_h)],
        2 => {
            let cell_w = (usable_w - gap) / 2.0;
            vec![
                (margin_mm, margin_mm, cell_w, usable_h),
                (margin_mm + cell_w + gap, margin_mm, cell_w, usable_h),
            ]
        }
        3 => {
            let cell_w = (usable_w - gap) / 2.0;
            let cell_h = (usable_h - gap) / 2.0;
            vec![
                // top row: 2 images
                (margin_mm, margin_mm + cell_h + gap, cell_w, cell_h),
                (
                    margin_mm + cell_w + gap,
                    margin_mm + cell_h + gap,
                    cell_w,
                    cell_h,
                ),
                // bottom row: 1 centered
                (margin_mm + usable_w / 4.0, margin_mm, cell_w, cell_h),
            ]
        }
        4 => {
            let cell_w = (usable_w - gap) / 2.0;
            let cell_h = (usable_h - gap) / 2.0;
            vec![
                (margin_mm, margin_mm + cell_h + gap, cell_w, cell_h),
                (
                    margin_mm + cell_w + gap,
                    margin_mm + cell_h + gap,
                    cell_w,
                    cell_h,
                ),
                (margin_mm, margin_mm, cell_w, cell_h),
                (margin_mm + cell_w + gap, margin_mm, cell_w, cell_h),
            ]
        }
        _ => vec![],
    }
}

// ─── Markdown parsing ───────────────────────────────────────────────────────

/// Parse a catalog Markdown file into a list of CatalogEntry structs.
/// Extracts image links and <!-- taken: TIMESTAMP --> HTML comments.
pub fn parse_catalog_markdown(content: &str) -> Vec<CatalogEntry> {
    let mut entries: Vec<CatalogEntry> = Vec::new();
    let parser = Parser::new(content);

    let mut pending_url: Option<String> = None;
    let mut pending_caption: Option<String> = None;

    for event in parser {
        match event {
            Event::Start(Tag::Link { dest_url, .. }) => {
                pending_url = Some(dest_url.to_string());
                pending_caption = Some(String::new());
            }
            Event::Text(text) => {
                if let Some(ref mut cap) = pending_caption {
                    cap.push_str(&text);
                }
            }
            Event::End(TagEnd::Link) => {
                if let (Some(url), Some(caption)) = (pending_url.take(), pending_caption.take()) {
                    entries.push(CatalogEntry {
                        url,
                        caption,
                        taken_at: None,
                    });
                }
            }
            Event::Html(html) | Event::InlineHtml(html) => {
                // Match <!-- taken: TIMESTAMP --> and attach to the most recent entry
                let trimmed = html.trim();
                if let Some(inner) = trimmed
                    .strip_prefix("<!-- taken:")
                    .and_then(|s| s.strip_suffix("-->"))
                {
                    let ts_str = inner.trim();
                    if let Ok(dt) = ts_str.parse::<DateTime<Utc>>() {
                        if let Some(entry) = entries.last_mut() {
                            entry.taken_at = Some(dt);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    entries
}

// ─── PDF export ──────────────────────────────────────────────────────────────

pub struct ExportOptions {
    pub page_format: PageFormat,
    pub layout_mode: LayoutMode,
    pub margin_mm: f64,
}

/// Scale (img_w, img_h) to fit within (cell_w, cell_h) preserving aspect ratio.
/// Returns (scaled_w, scaled_h).
fn scale_to_fit(img_w: u32, img_h: u32, cell_w: f64, cell_h: f64) -> (f64, f64) {
    let scale_x = cell_w / img_w as f64;
    let scale_y = cell_h / img_h as f64;
    let scale = scale_x.min(scale_y);
    (img_w as f64 * scale, img_h as f64 * scale)
}

/// Convert a file:// URL to a local file path.
fn url_to_path(url: &str) -> Option<String> {
    if let Ok(u) = url::Url::parse(url) {
        if u.scheme() == "file" {
            return u.to_file_path().ok()?.to_str().map(String::from);
        }
    }
    // Treat as a plain path
    Some(url.to_string())
}

pub fn export_pdf(
    markdown_path: &str,
    output_path: &str,
    options: &ExportOptions,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let content = std::fs::read_to_string(markdown_path)?;
    let entries = parse_catalog_markdown(&content);

    if entries.is_empty() {
        warn!("No image entries found in {}", markdown_path);
    }

    // Group into pages
    let page_groups: Vec<PageGroup> = match &options.layout_mode {
        LayoutMode::Fixed(n) => entries
            .chunks(*n as usize)
            .map(|chunk| PageGroup {
                entries: chunk.to_vec(),
            })
            .collect(),
        LayoutMode::Auto { threshold } => auto_group_pages(entries, *threshold),
    };

    let pw = options.page_format.width_mm;
    let ph = options.page_format.height_mm;
    let margin = options.margin_mm;

    let (doc, first_page, first_layer) =
        PdfDocument::new("Photo Catalog", Mm(pw as f32), Mm(ph as f32), "Layer 1");
    let font = doc.add_builtin_font(BuiltinFont::Helvetica)?;

    let mut page_index = 0usize;

    for group in &page_groups {
        let (page_ref, layer_ref) = if page_index == 0 {
            (first_page.clone(), first_layer.clone())
        } else {
            let (p, l) = doc.add_page(Mm(pw as f32), Mm(ph as f32), "Layer 1");
            (p, l)
        };
        page_index += 1;

        let layer = doc.get_page(page_ref).get_layer(layer_ref);
        let n = group.entries.len() as u8;
        let cells = calculate_grid_cells(pw, ph, margin, n);

        for (entry, (cell_x, cell_y, cell_w, cell_h)) in
            group.entries.iter().zip(cells.iter())
        {
            let path = match url_to_path(&entry.url) {
                Some(p) => p,
                None => {
                    warn!("Cannot resolve path for URL: {}", entry.url);
                    continue;
                }
            };

            let img_data = match ::image::open(&path) {
                Ok(img) => img,
                Err(e) => {
                    error!("Cannot open image {}: {}", path, e);
                    continue;
                }
            };

            let (iw, ih) = img_data.dimensions();
            let (scaled_w, scaled_h) = scale_to_fit(iw, ih, *cell_w, *cell_h);

            // Center the image within the cell
            let offset_x = cell_x + (cell_w - scaled_w) / 2.0;
            let offset_y = cell_y + (cell_h - scaled_h) / 2.0;

            // Convert image to RGB8 for embedding
            let rgb = img_data.to_rgb8();
            let raw: Vec<u8> = rgb.into_raw();

            let pdf_image = Image::from(ImageXObject {
                width: Px(iw as usize),
                height: Px(ih as usize),
                color_space: ColorSpace::Rgb,
                bits_per_component: ColorBits::Bit8,
                interpolate: true,
                image_data: raw,
                image_filter: None,
                clipping_bbox: None,
                smask: None,
            });

            // printpdf transform: scale in DPI-space. 1mm ≈ 2.8346 pt; image DPI is virtual 300.
            // We use Mm for the image size directly via the transform.
            let dpi = 300.0_f64;
            let scale_x = (scaled_w / 25.4) * dpi / iw as f64;
            let scale_y = (scaled_h / 25.4) * dpi / ih as f64;

            let transform = ImageTransform {
                translate_x: Some(Mm(offset_x as f32)),
                translate_y: Some(Mm(offset_y as f32)),
                scale_x: Some(scale_x as f32),
                scale_y: Some(scale_y as f32),
                ..Default::default()
            };

            pdf_image.add_to_layer(layer.clone(), transform);

            // Caption below the image
            let caption_y = offset_y - 4.0;
            if caption_y > 0.0 {
                layer.use_text(
                    entry.caption.as_str(),
                    7.0,
                    Mm(offset_x as f32),
                    Mm(caption_y as f32),
                    &font,
                );
            }
        }
    }

    let output_file = File::create(output_path)?;
    doc.save(&mut BufWriter::new(output_file))?;
    Ok(())
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_page_format() {
        let a4l = parse_page_format("A4-landscape").unwrap();
        assert!((a4l.width_mm - 297.0).abs() < 0.1);
        assert!((a4l.height_mm - 210.0).abs() < 0.1);

        let a4p = parse_page_format("A4-portrait").unwrap();
        assert!((a4p.width_mm - 210.0).abs() < 0.1);
        assert!((a4p.height_mm - 297.0).abs() < 0.1);
    }

    #[test]
    fn test_auto_group_pages_basic() {
        let make = |ts: &str| CatalogEntry {
            url: "file:///x.jpg".to_string(),
            caption: "x".to_string(),
            taken_at: Some(ts.parse::<DateTime<Utc>>().unwrap()),
        };

        let entries = vec![
            make("2024-07-05T14:00:00Z"),
            make("2024-07-05T14:05:00Z"),
            make("2024-07-05T14:08:00Z"),
            make("2024-07-05T14:09:00Z"),
            make("2024-07-05T16:00:00Z"), // gap > 10 min from previous
        ];
        let threshold = chrono::Duration::minutes(10);
        let pages = auto_group_pages(entries, threshold);
        assert_eq!(pages.len(), 2);
        assert_eq!(pages[0].entries.len(), 4);
        assert_eq!(pages[1].entries.len(), 1);
    }

    #[test]
    fn test_auto_group_pages_no_timestamp() {
        let entries = vec![
            CatalogEntry {
                url: "file:///a.jpg".to_string(),
                caption: "a".to_string(),
                taken_at: None,
            },
            CatalogEntry {
                url: "file:///b.jpg".to_string(),
                caption: "b".to_string(),
                taken_at: None,
            },
        ];
        let pages = auto_group_pages(entries, chrono::Duration::minutes(10));
        // Images without timestamps each get their own page
        assert_eq!(pages.len(), 2);
    }

    #[test]
    fn test_calculate_grid_cells_counts() {
        for n in 1u8..=4 {
            let cells = calculate_grid_cells(297.0, 210.0, 10.0, n);
            assert_eq!(cells.len(), n as usize);
        }
    }

    #[test]
    fn test_parse_catalog_markdown() {
        let md = r#"## 2024-07-05

- [a.jpg](file:///photos/a.jpg) <!-- taken: 2024-07-05T14:00:00Z -->
- [b.jpg](file:///photos/b.jpg) <!-- taken: 2024-07-05T15:00:00Z -->
"#;
        let entries = parse_catalog_markdown(md);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].url, "file:///photos/a.jpg");
        assert_eq!(entries[0].caption, "a.jpg");
        assert!(entries[0].taken_at.is_some());
        assert_eq!(entries[1].url, "file:///photos/b.jpg");
        assert!(entries[1].taken_at.is_some());
    }

    #[test]
    fn test_layout_mode_from_args() {
        assert!(matches!(
            LayoutMode::from_args("auto", 10).unwrap(),
            LayoutMode::Auto { .. }
        ));
        assert!(matches!(
            LayoutMode::from_args("2", 10).unwrap(),
            LayoutMode::Fixed(2)
        ));
        assert!(LayoutMode::from_args("5", 10).is_err());
        assert!(LayoutMode::from_args("bad", 10).is_err());
    }
}
