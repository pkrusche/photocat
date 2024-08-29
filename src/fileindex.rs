use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;
use std::fs::File;
use std::io::{Read, Result};
use std::path::Path;
use url::Url;

#[derive(Debug, Serialize, Deserialize)]
pub struct IndexFile {
    pub filename: String,
    pub url: String,
    pub sha256: String,
    pub created_at: DateTime<Utc>,
    pub modified_at: DateTime<Utc>,
    pub meta: Vec<MetaVariable>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub enum MetaValue {
    String(String),
    Int(i64),
    UInt(u64),
    Float(f64),
    Bool(bool),
    Date(DateTime<Utc>),
    Null,
}

impl MetaValue {
    pub fn string_type(&self) -> &str {
        match self {
            MetaValue::String(_) => "String",
            MetaValue::Int(_) => "Int",
            MetaValue::UInt(_) => "UInt",
            MetaValue::Float(_) => "Float",
            MetaValue::Bool(_) => "Bool",
            MetaValue::Date(_) => "Date",
            MetaValue::Null => "Null",
        }
    }
}

impl fmt::Display for MetaValue {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            MetaValue::String(s) => write!(f, "{}", s),
            MetaValue::Int(i) => write!(f, "{}", i),
            MetaValue::UInt(u) => write!(f, "{}", u),
            MetaValue::Float(fl) => write!(f, "{}", fl),
            MetaValue::Bool(b) => write!(f, "{}", b),
            MetaValue::Date(d) => write!(f, "{}", d),
            MetaValue::Null => write!(f, "NULL"),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MetaVariable {
    pub name: String,
    pub value: MetaValue,
}

/// Creates a new `IndexFile` instance.
///
/// # Arguments
///
/// * `name` - The name of the file.
///
/// # Returns
///
/// Returns a `Result` containing the `IndexFile` instance if successful, or an `std::io::Error` if the file does not exist.
/// Metadata are left empty by default.
/// ```
impl IndexFile {
    pub fn new(name: &str) -> Result<IndexFile> {
        let filename: String = std::fs::canonicalize(name)
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();
        let url = Url::from_file_path(&filename).unwrap().to_string();
        let sha256 = calculate_sha256_of_file(name, &url).unwrap();
        let created = File::open(name)?.metadata()?.created().unwrap();
        let modified = File::open(name)?.metadata()?.modified().unwrap();
        let created_dt: DateTime<Utc> = created.into();
        let modified_dt: DateTime<Utc> = modified.into();

        if !Path::new(&filename).is_file() {
            Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "File does not exist!",
            ))
        } else {
            Ok(IndexFile {
                url,
                filename,
                sha256,
                created_at: created_dt,
                modified_at: modified_dt,
                meta: Vec::new(),
            })
        }
    }
}

/// Hash a file, return result as string
fn calculate_sha256_of_file(name: &str, extra: &str) -> Result<String> {
    let mut hasher = Sha256::new();
    let mut file = File::open(name)?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)?;
    hasher.update(&buffer);
    hasher.update(extra);
    let result = hasher.finalize();
    Ok(format!("{:x}", result))
}
