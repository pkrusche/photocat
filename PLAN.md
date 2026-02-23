# Feature Plan: Markdown Catalog & PDF Export for Photocat

## Overview

Add two new subcommands to `photocat`:

1. **`catalog`** — Given a selection of images (via existing filters), generate a Markdown document containing hyperlinks to each image.
2. **`export-pdf`** — Convert a catalog Markdown file to a PDF with 1–4 images laid out per page, with configurable page format (default: A4 landscape).

---

## 1. New CLI Subcommands

### Current Architecture

The tool uses `clap` with a flat `Args` struct and a `ValueEnum` called `Action`. All filtering options (`-d`, `-D`, `-u`, `-s`, `-N`) are top-level arguments reused across actions.

### Proposed Changes to `main.rs`

Add two new variants to the `Action` enum:

```rust
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum, Debug)]
enum Action {
    Index,
    List,
    Show,
    Summarize,
    MetaColumns,
    Catalog,    // NEW
    ExportPdf,  // NEW
}
```

Add new CLI arguments to `Args`:

```rust
/// Output file path for catalog / PDF export
#[arg(short('o'), long)]
output: Option<String>,

/// Page format for PDF export (e.g. "A4-landscape", "A4-portrait", "letter-landscape")
#[arg(long, default_value_t = String::from("A4-landscape"))]
page_format: String,

/// Number of images per page in PDF: 1-4 or "auto" (groups by time proximity)
#[arg(long, default_value_t = String::from("auto"))]
images_per_page: String,

/// Time threshold in minutes for "auto" layout grouping (default: 10)
#[arg(long, default_value_t = 10)]
auto_group_minutes: u64,

/// Title for the catalog document
#[arg(long)]
catalog_title: Option<String>,

/// Include EXIF metadata (camera, lens, date) in catalog entries
#[arg(long)]
catalog_meta: Option<bool>,
```

---

## 2. Feature: `catalog` — Markdown Generation

### Module: `src/catalog.rs`

**Responsibility:** Accept a list of `IndexFile` records and produce a Markdown document.

### Markdown Output Format

```markdown
# Photo Catalog

*Generated on 2024-09-15*

## 2024-07-05

- [DSC_5632.jpg](file:///path/to/DSC_5632.jpg) <!-- taken: 2024-07-05T14:32:10Z -->
  Camera: Nikon Z6 | Lens: 85mm f/1.8 | ISO 400

- [DSC_5633.jpg](file:///path/to/DSC_5633.jpg) <!-- taken: 2024-07-05T14:33:45Z -->
  Camera: Nikon Z6 | Lens: 35mm f/1.8 | ISO 200

- [DSC_5700.jpg](file:///path/to/DSC_5700.jpg) <!-- taken: 2024-07-05T18:10:00Z -->

## 2024-07-06

- [DSC_5701.jpg](file:///path/to/DSC_5701.jpg) <!-- taken: 2024-07-06T09:00:00Z -->
```

Images are grouped by date (from the `DateTaken` meta field or `created_at` fallback). Each entry is a Markdown hyperlink to the image using its `url` field (which is already a `file://` URL). Optional EXIF metadata is appended when `--catalog-meta` is set.

**Timestamp comments**: Each entry includes an HTML comment with the precise timestamp (`<!-- taken: ... -->`). This is invisible when rendering the Markdown normally, but is parsed by the `export-pdf` command to power the auto-layout grouping feature (see Section 3.1).

### Implementation Sketch

```rust
// src/catalog.rs
use crate::fileindex::IndexFile;
use std::collections::BTreeMap;
use std::io::Write;

pub struct CatalogOptions {
    pub title: String,
    pub include_meta: bool,
}

pub fn generate_catalog(
    records: &[IndexFile],
    options: &CatalogOptions,
    writer: &mut dyn Write,
) -> std::io::Result<()> {
    // 1. Group records by date (day granularity)
    // 2. Write markdown header with title
    // 3. For each date group, write section heading
    // 4. For each record, write hyperlink + optional meta line
    Ok(())
}
```

### Integration in `main.rs`

```rust
if args.action == Action::Catalog {
    let mut records: Vec<IndexFile> = Vec::new();
    query_fileindex(
        &args.list_sha, &filenames, &args.list_url,
        &args.list_limit, &min_date, &max_date,
        |record| records.push(record),
    ).expect("Query failed");

    let options = CatalogOptions {
        title: args.catalog_title.unwrap_or("Photo Catalog".into()),
        include_meta: args.catalog_meta.unwrap_or(false),
    };

    let output_path = args.output.unwrap_or("catalog.md".into());
    let mut file = File::create(&output_path)?;
    catalog::generate_catalog(&records, &options, &mut file)?;
    println!("Catalog written to {}", output_path);
}
```

### Usage Examples

```bash
# Basic catalog of all July 2024 photos
photocat -l ./data catalog -d 2024-07-01 -D 2024-08-01 -o july-catalog.md

# With EXIF metadata
photocat -l ./data catalog -d 2024-07-01 --catalog-meta true -o july-catalog.md

# Custom title
photocat -l ./data catalog --catalog-title "Summer Vacation 2024" -o vacation.md
```

---

## 3. Feature: `export-pdf` — Markdown to PDF with Image Layout

### Module: `src/pdfexport.rs`

**Responsibility:** Parse a catalog Markdown file, load the referenced images, and render them into a PDF with a configurable layout.

### Approach: Use the `printpdf` Crate

The [`printpdf`](https://crates.io/crates/printpdf) crate is a pure-Rust PDF generation library. It supports:

- Custom page sizes (A4, Letter, arbitrary)
- Embedding JPEG and PNG images
- Text rendering with built-in fonts
- Full control over layout coordinates

This avoids any external dependencies like `wkhtmltopdf` or headless Chrome.

For parsing the Markdown to extract image links, use [`pulldown-cmark`](https://crates.io/crates/pulldown-cmark).

### New Dependencies in `Cargo.toml`

```toml
printpdf = "0.7"
pulldown-cmark = "0.12"
image = "0.25"           # for reading image dimensions & format conversion
```

### Page Format Parsing

```rust
pub struct PageFormat {
    pub width_mm: f64,
    pub height_mm: f64,
    pub name: String,
}

pub fn parse_page_format(spec: &str) -> Result<PageFormat, String> {
    // Parse strings like "A4-landscape", "A4-portrait", "letter-landscape"
    // A4 portrait:  210 x 297 mm
    // A4 landscape: 297 x 210 mm
    // Letter portrait:  215.9 x 279.4 mm
    // Letter landscape: 279.4 x 215.9 mm
}
```

### Layout Strategy

For N images per page (1–4), use a grid layout:

| `images_per_page` | Grid    | Description                              |
|-------------------|---------|------------------------------------------|
| 1                 | 1×1     | Single image, centered, maximum size     |
| 2                 | 1×2     | Two images side-by-side (landscape) or stacked (portrait) |
| 3                 | 2×2     | Top row: 2, bottom row: 1 centered       |
| 4                 | 2×2     | Four images in a 2×2 grid                |

Each cell gets equal space with configurable margins (default 10mm). Images are scaled to fit within their cell while preserving aspect ratio.

A small caption (filename, date) is rendered below each image.

### 3.1 Auto Layout Mode (`--images-per-page auto`)

When `--images-per-page` is set to `auto` (the default), the PDF exporter dynamically decides how many images go on each page based on **temporal proximity**: images taken within `--auto-group-minutes` (default: 10 minutes) of each other are grouped onto the same page, up to a maximum of 4.

This reflects real-world shooting patterns — a burst of shots at one location gets a dense 4-up page, while a single hero shot from later in the day gets a full page to itself.

#### Algorithm

The input is a list of `CatalogEntry` structs parsed from the Markdown, each carrying a URL, caption, and an `Option<DateTime<Utc>>` timestamp extracted from the `<!-- taken: ... -->` HTML comment.

```rust
/// A single image entry parsed from the catalog markdown
pub struct CatalogEntry {
    pub url: String,
    pub caption: String,
    pub taken_at: Option<DateTime<Utc>>,
}

/// A page to be rendered, containing 1-4 images
pub struct PageGroup {
    pub entries: Vec<CatalogEntry>,  // 1..=4 entries
}

/// Group catalog entries into pages using temporal proximity.
///
/// Consecutive images taken within `threshold` of each other are placed
/// on the same page (up to 4). Images without timestamps get their own page.
pub fn auto_group_pages(
    entries: Vec<CatalogEntry>,
    threshold: chrono::Duration,
) -> Vec<PageGroup> {
    let mut pages: Vec<PageGroup> = Vec::new();
    let mut current_group: Vec<CatalogEntry> = Vec::new();

    for entry in entries {
        if current_group.is_empty() {
            current_group.push(entry);
            continue;
        }

        let should_extend = match (&current_group.last().unwrap().taken_at, &entry.taken_at) {
            (Some(prev), Some(curr)) => {
                // Time between consecutive images is within threshold
                (*curr - *prev).abs() <= threshold
            }
            _ => false, // No timestamp → don't group
        };

        if should_extend && current_group.len() < 4 {
            current_group.push(entry);
        } else {
            // Flush current group as a page, start new group
            pages.push(PageGroup { entries: std::mem::take(&mut current_group) });
            current_group.push(entry);
        }
    }

    if !current_group.is_empty() {
        pages.push(PageGroup { entries: current_group });
    }

    pages
}
```

#### Grouping Rules

| Scenario | Result |
|----------|--------|
| 4 images all within 10 min of each other | 1 page, 2×2 grid |
| 6 images within 10 min | Page 1: 4 images (2×2), Page 2: 2 images (1×2) |
| 2 images 3 min apart, then 1 image 2 hours later | Page 1: 2 images (1×2), Page 2: 1 image (full page) |
| Image with no timestamp | Gets its own page (1×1) |
| 3 images: A→B is 8 min, B→C is 8 min (A→C is 16 min) | All 3 on one page — grouping is **consecutive**, not pairwise |

The consecutive-pair approach is intentional: it mirrors how photographers work through a scene, where each shot is close in time to the previous one even if the first and last are far apart.

#### Interaction with Fixed Mode

The `--images-per-page` argument accepts either `"auto"` or a number `"1"`–`"4"`:

```rust
pub enum LayoutMode {
    Auto { threshold: chrono::Duration },
    Fixed(u8),  // 1-4
}

impl LayoutMode {
    pub fn from_args(images_per_page: &str, auto_group_minutes: u64) -> Result<Self, String> {
        match images_per_page {
            "auto" => Ok(LayoutMode::Auto {
                threshold: chrono::Duration::minutes(auto_group_minutes as i64),
            }),
            n => {
                let count: u8 = n.parse().map_err(|_| format!(
                    "Invalid --images-per-page: '{}'. Use 1-4 or 'auto'.", n
                ))?;
                if count < 1 || count > 4 {
                    return Err("--images-per-page must be 1-4 or 'auto'".into());
                }
                Ok(LayoutMode::Fixed(count))
            }
        }
    }
}
```

In `export_pdf`, the dispatch becomes:

```rust
let page_groups: Vec<PageGroup> = match options.layout_mode {
    LayoutMode::Fixed(n) => {
        // Chunk all entries into groups of n
        entries.chunks(n as usize)
            .map(|chunk| PageGroup { entries: chunk.to_vec() })
            .collect()
    }
    LayoutMode::Auto { threshold } => {
        auto_group_pages(entries, threshold)
    }
};

// Each PageGroup knows its own size, so grid layout adapts per page
for group in &page_groups {
    let cells = calculate_grid_cells(
        page_format.width_mm, page_format.height_mm,
        margin_mm, group.entries.len() as u8,
    );
    // ... render page with these cells
}
```

#### Example Scenarios

**Scenario: Beach vacation day**

Images sorted by time:
- 10:00 – panorama shot (standalone)
- 14:02, 14:03, 14:05, 14:08 – group at tide pools
- 14:09, 14:11 – still within threshold of the burst
- 17:30 – sunset (standalone)

Result with `--auto-group-minutes 10`:
- Page 1: 1 image (1×1) — 10:00 panorama
- Page 2: 4 images (2×2) — 14:02, 14:03, 14:05, 14:08
- Page 3: 2 images (1×2) — 14:09, 14:11
- Page 4: 1 image (1×1) — 17:30 sunset

### Implementation Sketch

```rust
// src/pdfexport.rs
use printpdf::*;
use std::path::Path;

pub struct ExportOptions {
    pub page_format: PageFormat,
    pub layout_mode: LayoutMode,  // Auto or Fixed(1-4)
    pub margin_mm: f64,           // default 10.0
}

pub fn export_pdf(
    markdown_path: &str,
    output_path: &str,
    options: &ExportOptions,
) -> Result<(), Box<dyn std::error::Error>> {
    // 1. Parse markdown, extract image URLs and captions
    //    - Use pulldown-cmark to find all links
    //    - Convert file:// URLs to local paths
    //
    // 2. Create PDF document with printpdf
    //
    // 3. For each page-worth of images:
    //    a. Add a new page with the specified dimensions
    //    b. Calculate grid cell positions based on images_per_page
    //    c. For each image in the batch:
    //       - Load with `image` crate, get dimensions
    //       - Scale to fit cell while preserving aspect ratio
    //       - Embed into PDF page at calculated position
    //       - Add filename caption below image
    //
    // 4. Save PDF to output_path
    Ok(())
}

/// Calculate (x, y, max_width, max_height) for each cell in the grid
fn calculate_grid_cells(
    page_width_mm: f64,
    page_height_mm: f64,
    margin_mm: f64,
    images_per_page: u8,
) -> Vec<(f64, f64, f64, f64)> {
    let usable_w = page_width_mm - 2.0 * margin_mm;
    let usable_h = page_height_mm - 2.0 * margin_mm;
    let gap = 5.0; // mm between cells

    match images_per_page {
        1 => vec![(margin_mm, margin_mm, usable_w, usable_h)],
        2 => {
            let cell_w = (usable_w - gap) / 2.0;
            vec![
                (margin_mm, margin_mm, cell_w, usable_h),
                (margin_mm + cell_w + gap, margin_mm, cell_w, usable_h),
            ]
        }
        3 | 4 => {
            let cell_w = (usable_w - gap) / 2.0;
            let cell_h = (usable_h - gap) / 2.0;
            let mut cells = vec![
                (margin_mm, margin_mm, cell_w, cell_h),
                (margin_mm + cell_w + gap, margin_mm, cell_w, cell_h),
            ];
            if images_per_page == 3 {
                // Bottom row: single centered image
                cells.push((margin_mm + usable_w / 4.0, margin_mm + cell_h + gap, cell_w, cell_h));
            } else {
                cells.push((margin_mm, margin_mm + cell_h + gap, cell_w, cell_h));
                cells.push((margin_mm + cell_w + gap, margin_mm + cell_h + gap, cell_w, cell_h));
            }
            cells
        }
        _ => vec![],
    }
}
```

### Integration in `main.rs`

```rust
if args.action == Action::ExportPdf {
    let input = args.photo_location.first()
        .expect("Please provide a markdown file path");
    let output_path = args.output.unwrap_or("catalog.pdf".into());
    let page_format = pdfexport::parse_page_format(&args.page_format)?;
    let layout_mode = pdfexport::LayoutMode::from_args(
        &args.images_per_page, args.auto_group_minutes
    )?;

    let options = pdfexport::ExportOptions {
        page_format,
        layout_mode,
        margin_mm: 10.0,
    };

    pdfexport::export_pdf(input, &output_path, &options)?;
    println!("PDF written to {}", output_path);
}
```

### Usage Examples

```bash
# Default: A4 landscape, auto layout (groups by 10-min proximity)
photocat -l ./data export-pdf catalog.md -o catalog.pdf

# Auto with 5-minute grouping threshold
photocat -l ./data export-pdf catalog.md -o catalog.pdf --auto-group-minutes 5

# Fixed: always 4 images per page on letter paper
photocat -l ./data export-pdf catalog.md -o catalog.pdf \
    --page-format letter-landscape --images-per-page 4

# Fixed: single large image per page, portrait
photocat -l ./data export-pdf catalog.md -o catalog.pdf \
    --page-format A4-portrait --images-per-page 1
```

---

## 4. File Changes Summary

| File                  | Change                                              |
|-----------------------|-----------------------------------------------------|
| `Cargo.toml`          | Add `printpdf`, `pulldown-cmark`, `image` deps      |
| `src/main.rs`         | Add `Catalog`/`ExportPdf` actions, new CLI args, dispatch logic |
| `src/catalog.rs`      | **New** — Markdown catalog generation                |
| `src/pdfexport.rs`    | **New** — PDF export with image grid layout          |

---

## 5. Implementation Order

1. **`src/catalog.rs`** — Implement Markdown generation with `<!-- taken: ... -->` timestamp comments. Wire up the `Catalog` action. Test with existing indexed photos.
2. **`src/pdfexport.rs` — types & parsing** — Define `CatalogEntry`, `PageGroup`, `LayoutMode`, `PageFormat`. Implement Markdown parsing (extract URLs + timestamps from HTML comments using `pulldown-cmark`). Write unit tests for parsing.
3. **`src/pdfexport.rs` — auto-grouping** — Implement `auto_group_pages` with the consecutive-pair temporal proximity algorithm. Write unit tests covering all grouping scenarios (bursts, gaps, missing timestamps, exactly-at-threshold).
4. **`src/pdfexport.rs` — grid layout** — Implement `calculate_grid_cells` and page format parsing. Unit tests for layout math.
5. **PDF image embedding** — Integrate `printpdf` + `image` crate for loading and embedding images. Handle JPEG (direct) and other formats (convert to PNG).
6. **End-to-end wiring** — Connect `ExportPdf` action in `main.rs`, parse `LayoutMode` from CLI args.
7. **Tests** — Add integration tests that generate a catalog from test data and export to PDF, verifying both auto and fixed layout modes produce correct page counts.

---

## 6. Edge Cases & Considerations

- **Missing images**: If a linked file no longer exists at export time, log a warning and skip it (or render a placeholder).
- **Large images**: Scale down before embedding to keep PDF file size reasonable. Consider a `--max-resolution` flag.
- **Non-JPEG formats**: RAW/HEIC/TIFF files need conversion. The `image` crate handles PNG/JPEG/TIFF; for HEIC/RAW, either skip or shell out to ImageMagick/`exiftool -b -ThumbnailImage`.
- **Relative vs absolute paths**: The catalog Markdown uses `file://` URLs (already absolute from `IndexFile.url`), so path resolution is straightforward.
- **Caption text**: Use a built-in PDF font (Helvetica) to avoid font embedding complexity.
- **Auto-layout with missing timestamps**: Images without a `<!-- taken: ... -->` comment are treated as temporal singletons — they always start a new page and are never grouped with neighbors. This is conservative and avoids incorrectly merging unrelated images.
- **Auto-layout threshold boundary**: The comparison uses `<=` (inclusive), so images exactly 10 minutes apart are grouped together. This matches user intuition ("within 10 minutes").
- **Manually edited Markdown**: If a user reorders entries in the Markdown file, auto-layout still works — it groups based on the embedded timestamps, not file order. However, the visual page order follows the Markdown order, so reordering can produce pages where timestamps jump around. This is by design since the user chose to reorder.
- **Mixed auto/fixed in one export**: Not supported. A single `--images-per-page` value applies to the entire PDF. Users who want different layouts for different sections should split into multiple catalogs.
