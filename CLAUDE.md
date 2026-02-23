# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
cargo build                # Debug build
cargo build --release      # Release build
cargo test                 # Run all tests
cargo test --all-features  # Run tests with all features
cargo fmt                  # Format code
cargo fmt --check          # Check formatting (CI)
```

## Architecture

**Photocat** is a CLI tool that catalogs photo EXIF metadata into a DuckDB database. Files are identified by SHA256 hash. The tool runs an external `exiftool` command (configurable via `--meta-cmd`) on each photo to extract JSON metadata.

### Data flow for `index`

1. Walk directory, compute SHA256 for each media file (`fileindex.rs`)
2. Run `--meta-cmd` on each file (stdin→stdout JSON); optionally merge with existing `{sha256}.json`
3. Store result as `{datapath}/{sha256}.json` and in DuckDB (`indexdb.rs`)
4. Concurrent processing via `consume_concurrently()` in `processing.rs` (Tokio + progress bar)

### Metadata extraction (`meta.sql`)

`meta.sql` is a SQL template (included at compile time via `include_str!`) that uses DuckDB's `read_json_auto()` to read all `data/*.json` files and extract/normalize EXIF fields into a temporary `meta` table. It handles multiple timestamp formats including millisecond-precision dates and infers lens info for iPhones. Adding a new metadata field means editing this SQL.

### Database

Single DuckDB connection stored as `OnceCell<Arc<Mutex<Connection>>>` in `indexdb.rs`. The `fileindex` table holds file identity; metadata is queried from JSON files via `meta.sql` at query time (not stored as columns).

### Summarization

`FileIndexSummarizer` trait in `summarystats.rs` — implement to add new summary types. `DateSummary` renders a colored grid histogram by date. `ValueCounter` produces frequency tables. Both are registered in `SummaryStats::new()` based on `--summary-options`.

### Metadata value mapping

`data/mapping.toml` (git-tracked) allows normalizing inconsistent EXIF values at runtime without code changes:
```toml
[[mapping]]
variable = 'Lens'
match_values = ['15 mm f/4.5']
assign_value = '15mm f/4.5'
```

## Key conventions

- Files are always identified by SHA256, not by filename/path
- Per-file metadata lives at `{library}/{sha256}.json`; the DuckDB at `{library}/photocat.db`
- All timestamps stored in UTC; `meta.sql` tries multiple date formats
- `--meta-merge` flag merges new metadata with existing JSON rather than overwriting
- The `Action` enum in `main.rs` drives all command dispatch
