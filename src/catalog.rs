use crate::fileindex::{IndexFile, MetaValue};
use chrono::{DateTime, Utc};
use std::collections::BTreeMap;
use std::io::{Result, Write};

pub struct CatalogOptions {
    pub title: String,
    pub include_meta: bool,
}

/// Extract the DateTaken timestamp from an IndexFile's metadata, falling back to created_at.
fn get_taken_at(record: &IndexFile) -> DateTime<Utc> {
    for var in &record.meta {
        if var.name == "DateTaken" {
            if let MetaValue::Date(dt) = &var.value {
                return *dt;
            }
        }
    }
    record.created_at
}

/// Format a line of EXIF metadata for the catalog entry.
fn format_meta_line(record: &IndexFile) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();

    let mut camera = String::new();
    for var in &record.meta {
        if var.name == "Make" {
            if let MetaValue::String(s) = &var.value {
                camera.push_str(s);
            }
        }
        if var.name == "Model" {
            if let MetaValue::String(s) = &var.value {
                if !camera.is_empty() {
                    camera.push(' ');
                }
                camera.push_str(s);
            }
        }
    }
    if !camera.is_empty() {
        parts.push(format!("Camera: {}", camera));
    }

    for var in &record.meta {
        if var.name == "Lens" || var.name == "LensModel" || var.name == "LensInferred" {
            if let MetaValue::String(s) = &var.value {
                if !s.is_empty() {
                    parts.push(format!("Lens: {}", s));
                    break;
                }
            }
        }
    }

    for var in &record.meta {
        if var.name == "ISO" {
            match &var.value {
                MetaValue::Int(i) => {
                    parts.push(format!("ISO {}", i));
                }
                MetaValue::UInt(u) => {
                    parts.push(format!("ISO {}", u));
                }
                MetaValue::String(s) if !s.is_empty() => {
                    parts.push(format!("ISO {}", s));
                }
                _ => {}
            }
            break;
        }
    }

    if parts.is_empty() {
        None
    } else {
        Some(parts.join(" | "))
    }
}

/// Extract just the filename portion from a file:// URL or path.
fn filename_from_url(url: &str) -> &str {
    url.rsplit('/').next().unwrap_or(url)
}

pub fn generate_catalog(
    records: &[IndexFile],
    options: &CatalogOptions,
    writer: &mut dyn Write,
) -> Result<()> {
    // Group records by date (day granularity), preserving order within each day.
    let mut by_date: BTreeMap<String, Vec<&IndexFile>> = BTreeMap::new();
    for record in records {
        let taken_at = get_taken_at(record);
        let day_key = taken_at.format("%Y-%m-%d").to_string();
        by_date.entry(day_key).or_default().push(record);
    }
    for entries in by_date.values_mut() {
        entries.sort_by_key(|record| get_taken_at(record));
    }

    let today = Utc::now().format("%Y-%m-%d");
    writeln!(writer, "# {}", options.title)?;
    writeln!(writer)?;
    writeln!(writer, "*Generated on {}*", today)?;

    for (day, entries) in &by_date {
        writeln!(writer)?;
        writeln!(writer, "## {}", day)?;
        writeln!(writer)?;

        for record in entries {
            let taken_at = get_taken_at(record);
            let filename = filename_from_url(&record.url);
            let timestamp = taken_at.format("%Y-%m-%dT%H:%M:%SZ");

            writeln!(
                writer,
                "- [{}]({}) <!-- taken: {} -->",
                filename, record.url, timestamp
            )?;

            if options.include_meta {
                if let Some(meta_line) = format_meta_line(record) {
                    writeln!(writer, "  {}", meta_line)?;
                }
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fileindex::MetaVariable;

    fn make_record(url: &str, taken: &str) -> IndexFile {
        let taken_at: DateTime<Utc> = taken.parse().unwrap();
        IndexFile {
            filename: url.to_string(),
            url: url.to_string(),
            sha256: "abc".to_string(),
            created_at: taken_at,
            modified_at: taken_at,
            meta: vec![MetaVariable {
                name: "DateTaken".to_string(),
                value: MetaValue::Date(taken_at),
            }],
        }
    }

    #[test]
    fn test_generate_catalog_groups_by_day() {
        let records = vec![
            make_record("file:///photos/a.jpg", "2024-07-05T14:00:00Z"),
            make_record("file:///photos/b.jpg", "2024-07-05T15:00:00Z"),
            make_record("file:///photos/c.jpg", "2024-07-06T09:00:00Z"),
        ];
        let options = CatalogOptions {
            title: "Test".to_string(),
            include_meta: false,
        };
        let mut buf: Vec<u8> = Vec::new();
        generate_catalog(&records, &options, &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();

        assert!(output.contains("## 2024-07-05"));
        assert!(output.contains("## 2024-07-06"));
        assert!(output.contains("<!-- taken: 2024-07-05T14:00:00Z -->"));
        assert!(output.contains("<!-- taken: 2024-07-06T09:00:00Z -->"));
    }

    #[test]
    fn test_generate_catalog_sorts_within_day() {
        let records = vec![
            make_record("file:///photos/late.jpg", "2024-07-05T15:00:00Z"),
            make_record("file:///photos/early.jpg", "2024-07-05T14:00:00Z"),
        ];
        let options = CatalogOptions {
            title: "Test".to_string(),
            include_meta: false,
        };
        let mut buf: Vec<u8> = Vec::new();
        generate_catalog(&records, &options, &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();

        let early_idx = output.find("early.jpg").unwrap();
        let late_idx = output.find("late.jpg").unwrap();
        assert!(early_idx < late_idx);
    }
}
