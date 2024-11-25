use chrono::{DateTime, Utc};
use duckdb::types::FromSql;
/// Module to maintain the main index database, which is
/// a duckdb file. It stores an entry for each file, giving its
/// name / URL and sha256.
use duckdb::{params_from_iter, types::ValueRef, Connection, Error, ToSql};
use itertools::Itertools;
use log::{debug, error, info, warn};
use once_cell::sync::OnceCell;
use serde_json::json;
use shlex::try_quote;
use std::sync::{Arc, Mutex};

use crate::fileindex::{self, IndexFile, MetaValue, MetaVariable};
use crate::jsonmeta;
use crate::variablemapping::{self, apply_mappings};

use duckdb::Result;
use std::fs::File;
use std::io::{BufReader, Write};
use std::process::Command;

static DBPATH: OnceCell<Arc<String>> = OnceCell::new();
static DB: OnceCell<Arc<Mutex<Connection>>> = OnceCell::new();
static MAPPINGS: OnceCell<Arc<variablemapping::Mappings>> = OnceCell::new();

/// Helper to split a SQL string into statements and run
fn run_sql(conn: &Connection, sql_str: &str) -> Result<usize, duckdb::Error> {
    let sql_statements = sql_str.split(';');
    let mut result: usize = 0;
    for statement in sql_statements {
        if !statement.trim().is_empty() {
            result = conn.execute(statement, [])?;
        }
    }
    Ok(result)
}

/// Set up the database connection
/// This initializes the global singleton DB and DBPATH variables
///
/// The DB path contains the following:
/// - a DuckDB file named photocat.db
/// - JSON files with metadata for each indexed entry (if these were created when indexing)
///
/// Since we rely on the JSON module in duckdb, we load and try to install.
pub fn init_connection(path: &str) {
    DBPATH
        .set(Arc::new(String::from(path)))
        .expect("Cannot initialize DB path");
    DB.set(Arc::new(Mutex::new(
        Connection::open(std::path::Path::new(path).join("photocat.db"))
            .expect("Failed to open DuckDB connection"),
    )))
    .expect("Cannot (re)initialize database connection.");
    let conn = DB.get().unwrap().lock().unwrap();

    let mappings = variablemapping::load_mappings(
        std::path::Path::new(path)
            .join("mapping.toml")
            .to_str()
            .unwrap(),
    );
    if let Ok(mappings) = mappings {
        info!("Loaded {} mappings from data folder.", mappings.len());
        MAPPINGS
            .set(Arc::new(mappings))
            .expect("Cannot initialize mappings.");
    }

    conn.execute(
        "CREATE TABLE IF NOT EXISTS fileindex (
            filename TEXT NOT NULL,
            url TEXT NOT NULL,
            sha256 TEXT NOT NULL,
            created_at TIMESTAMP NOT NULL,
            modified_at TIMESTAMP NOT NULL
            ); ",
        [],
    )
    .expect("Failed to create fileindex table");
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_sha256 ON fileindex (sha256);",
        [],
    )
    .expect("Failed to create index on sha256");
    let can_load_json = conn.execute("LOAD JSON;", []);
    if can_load_json.is_err() {
        warn!(
            "Cannot load the JSON module for DuckDB, trying to install: {:?}",
            can_load_json.err()
        );
        conn.execute("INSTALL 'JSON';", [])
            .expect("Cannot install JSON module for DuckDB");
        conn.execute("LOAD JSON;", [])
            .expect("Cannot load JSON module in DuckDB");
    }
    {
        // run JSON ingestion
        let sql_str = include_str!("meta.sql").replace("{{datapath}}", path);
        if let Err(e) = run_sql(&conn, &sql_str) {
            error!("Failed to run meta SQL command {}", e);
        }
    }
}

/// Add a single file to the index
///
/// Args:
/// path: local path to the file
/// meta_cmd: Command to produce metadata
/// meta_merge: set to true to merge metadata objects, false to overwrite
pub fn index_file(
    path: String,
    meta_cmd: Option<String>,
    meta_merge: bool,
) -> Result<(), std::io::Error> {
    let fileinfo = fileindex::IndexFile::new(path.as_str()).unwrap();

    // this bit blocks the DuckDB connection
    {
        // Get the DuckDB connection
        let conn = DB.get().unwrap().lock().unwrap();

        // Insert the fileinfo into the database
        let mut stmt = conn.prepare("INSERT INTO fileindex \
                                                (filename, url, sha256, created_at, modified_at) \
                                                SELECT ?, ?, ?, ?, ? \
                                                WHERE NOT EXISTS (SELECT 1 FROM fileindex WHERE sha256 = ?)")
                                                .expect("Failed to prepare statement");
        let inserted = stmt
            .execute(&[
                &fileinfo.filename,
                &fileinfo.url,
                &fileinfo.sha256,
                &fileinfo.created_at.to_string(),
                &fileinfo.modified_at.to_string(),
                &fileinfo.sha256,
            ])
            .expect("Failed to insert fileinfo into database");
        debug!(
            "Inserted {} rows for {} / {}",
            inserted, fileinfo.filename, fileinfo.sha256
        );
    }

    if let Some(meta_cmd) = meta_cmd {
        // Quote the json_path for shell execution
        let quoted_file_path: String = try_quote(&fileinfo.filename).unwrap().to_string();
        // Run the meta_cmd in the shell
        let output = Command::new("sh")
            .arg("-c")
            .arg(format!("cat {} | {}", quoted_file_path, meta_cmd))
            .output()
            .expect("Failed to execute meta_cmd");

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let json = serde_json::from_str(&stdout);
            let mut json_val;
            if json.is_err() {
                json_val = serde_json::Value::Null;
                error!(
                    "Cannot parse output for {} {}: {} / {}",
                    meta_cmd, quoted_file_path, stdout, stderr
                );
            } else {
                json_val = json.unwrap();
            }
            // Create path of JSON file as DBPATH/fileinfo.sha256.json
            let json_path = std::path::Path::new(DBPATH.get().unwrap().as_str())
                .join(format!("{}.json", fileinfo.sha256.as_str()));

            // merge if requested
            if meta_merge {
                if let Ok(file) = File::open(&json_path) {
                    let reader = BufReader::new(file);
                    // TODO log error when we cannot read the current value
                    let current_json = serde_json::from_reader(reader);
                    if current_json.is_err() {
                        error!(
                            "Cannot parse current JSON for {}: {:?}",
                            fileinfo.sha256,
                            current_json.err().unwrap()
                        );
                    } else {
                        let mut current_json_val = current_json.unwrap();
                        jsonmeta::merge(&mut current_json_val, json_val);
                        json_val = current_json_val;
                    }
                }
            }
            // flatten single element arrays (such as the ones returned by exiftool)
            // to pass an array, assign it inside a top-level object
            while let serde_json::Value::Array(ref mut arr) = json_val {
                if arr.len() == 1 {
                    json_val = arr.remove(0);
                } else {
                    break;
                }
            }
            match json_val {
                serde_json::Value::Object(ref mut obj) => {
                    obj.insert(
                        String::from("sha256"),
                        serde_json::Value::String(fileinfo.sha256),
                    );
                }
                _ => {
                    json_val = json!({
                        "sha256": fileinfo.sha256,
                        "data": json_val,
                    });
                }
            }
            // Write json_val into file at json_path
            let mut file = File::create(&json_path)?;
            file.write_all(json_val.to_string().as_bytes())?;
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            error!("Failed to execute meta_cmd: {}", stderr);
        }
    }

    Ok(())
}

/// Return true if we have a metadata table
pub fn has_meta() -> bool {
    let conn = DB.get().expect("Database not initialized");
    let conn = conn.lock().unwrap();
    let table_exists: bool = conn
        .query_row(
            "SELECT EXISTS (SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'meta')",
            [],
            |row| row.get(0),
        )
        .unwrap();
    table_exists
}

// return dictionary of columns and types in meta table
pub fn get_meta_columns() -> Result<Vec<(i32, String, String)>> {
    assert!(has_meta(), "No metadata table present");
    let conn = DB.get().expect("Database not initialized");
    let conn = conn.lock().unwrap();
    let mut columns = Vec::new();
    let mut stmt = conn.prepare("PRAGMA table_info(meta)")?;
    let rows = stmt.query_map([], |row| {
        let cid: i32 = row.get(0)?;
        let name: String = row.get(1)?;
        let type_: String = row.get(2)?;
        Ok((cid, name, type_))
    })?;
    for row in rows {
        let (cid, name, type_) = row?;
        columns.push((cid, name, type_));
    }
    Ok(columns.into_iter().sorted_by_key(|x| x.0).collect())
}

/// Create vector of file index entries from the database based on the provided filters.
///
/// # Arguments
///
/// * `sha256s` - Optional string containing comma-separated SHA256 values to filter by.
/// * `filename` - Optional string containing the filename to filter by.
/// * `url` - Optional string containing the URL to filter by.
/// * `limit` - Optional limit on the number of results to retrieve.
///
pub fn query_fileindex(
    sha256s: &Option<String>,
    filename: &Option<String>,
    url: &Option<String>,
    limit: &Option<usize>,
    min_date: &Option<chrono::DateTime<Utc>>,
    max_date: &Option<chrono::DateTime<Utc>>,
    mut callback: impl FnMut(IndexFile),
) -> Result<(), Error> {
    let has_meta = has_meta();
    let meta_columns = get_meta_columns();
    let mut query;
    if has_meta {
        query =
            String::from("SELECT filename, url, fileindex.sha256, created_at, modified_at, meta.* FROM fileindex JOIN meta ON (fileindex.sha256 = meta.sha256)");
    } else {
        query =
            String::from("SELECT filename, url, sha256, created_at, modified_at FROM fileindex");
    }

    let mut params: Vec<Box<dyn ToSql>> = Vec::new();
    let mut has_params = false;

    let sha256_vec: Vec<&str> = sha256s
        .as_ref()
        .map(|s| s.split(',').collect())
        .unwrap_or_else(Vec::new);
    if !sha256_vec.is_empty() {
        let placeholders = sha256_vec.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        query.push_str(" WHERE sha256 IN (");
        query.push_str(&placeholders);
        query.push_str(")");
        for p in sha256_vec.iter().map(|s| Box::new(String::from(*s))) {
            params.push(p);
        }
        has_params = true;
    }

    let filename_format_string: String;
    if let Some(filename) = filename {
        if has_params {
            query.push_str(" AND filename LIKE ?");
        } else {
            query.push_str(" WHERE filename LIKE ?");
        }
        filename_format_string = format!("%{}%", &filename);
        params.push(Box::new(String::from(&filename_format_string)));
        has_params = true;
    }

    let url_format_string: String;
    if let Some(url) = url {
        if has_params {
            query.push_str(" AND url LIKE ?");
        } else {
            query.push_str(" WHERE url LIKE ?");
        }
        url_format_string = format!("%{}%", &url);
        params.push(Box::new(String::from(&url_format_string)));
        has_params = true;
    }

    let min_date_str = if let Some(min_date) = min_date {
        format!(
            " created_at >= CAST('{}' AS TIMESTAMP) AND modified_at >= CAST('{}' AS TIMESTAMP)",
            min_date.to_rfc3339(),
            min_date.to_rfc3339()
        )
    } else {
        String::new()
    };
    if !min_date_str.is_empty() {
        if has_params {
            query.push_str(" AND");
        } else {
            query.push_str(" WHERE");
        }
        query.push_str(&min_date_str);
        has_params = true;
    }

    let max_date_str = if let Some(max_date) = max_date {
        format!(
            " created_at <= CAST('{}' AS TIMESTAMP) AND modified_at <= CAST('{}' AS TIMESTAMP)",
            max_date.to_rfc3339(),
            max_date.to_rfc3339()
        )
    } else {
        String::new()
    };
    if !max_date_str.is_empty() {
        if has_params {
            query.push_str(" AND");
        } else {
            query.push_str(" WHERE");
        }
        query.push_str(&max_date_str);
        #[allow(unused_assignments)]
        {
            has_params = true;
        }
    }

    let limit_str: String;
    if let Some(limit) = limit {
        limit_str = format!("LIMIT {limit}");
        query.push_str(&limit_str);
    }

    query.push_str(" ORDER BY CREATED_AT");

    {
        let conn = DB.get().expect("Database not initialized");
        let conn = conn.lock().unwrap();
        let mut stmt = conn.prepare(&query)?;
        debug!("{:?}", query);

        // Convert the params to a slice of references
        let params_refs: Vec<&dyn ToSql> = params.iter().map(|p| &**p).collect();

        let indexfile_iter = stmt.query_map(params_from_iter(params_refs), |row| {
            Ok({
                let filename: String = row.get(0).expect("Failed to get filename");
                let url: String = row.get(1).expect("Failed to get url");
                let sha256: String = row.get(2).expect("Failed to get sha256");
                let created_at: DateTime<chrono::Utc> =
                    row.get(3).expect("Failed to get created_at");
                let modified_at: DateTime<chrono::Utc> =
                    row.get(4).expect("Failed to get modified_at");
                let mut meta = Vec::new();

                if let Ok(ref meta_columns) = meta_columns {
                    let mut idx = 5;
                    for col in meta_columns {
                        let value = row.get_ref_unwrap(idx);
                        match value {
                            ValueRef::Null => {
                                meta.push(MetaVariable {
                                    name: col.1.clone(),
                                    value: MetaValue::Null,
                                });
                            }
                            ValueRef::Boolean(b) => {
                                meta.push(MetaVariable {
                                    name: col.1.clone(),
                                    value: MetaValue::Bool(b),
                                });
                            }
                            ValueRef::Double(_) | ValueRef::Float(_) => {
                                meta.push(MetaVariable {
                                    name: col.1.clone(),
                                    value: MetaValue::Float(f64::column_result(value).unwrap()),
                                });
                            }
                            ValueRef::Int(_) | ValueRef::BigInt(_) => {
                                meta.push(MetaVariable {
                                    name: col.1.clone(),
                                    value: MetaValue::Int(i64::column_result(value).unwrap()),
                                });
                            }
                            ValueRef::UInt(_) | ValueRef::UBigInt(_) => {
                                meta.push(MetaVariable {
                                    name: col.1.clone(),
                                    value: MetaValue::UInt(u64::column_result(value).unwrap()),
                                });
                            }
                            ValueRef::Text(s) => {
                                let decoded_string = String::from_utf8_lossy(s).to_string();
                                meta.push(MetaVariable {
                                    name: col.1.clone(),
                                    value: MetaValue::String(decoded_string),
                                });
                            }
                            ValueRef::Timestamp(_, _)
                            | ValueRef::Date32(_)
                            | ValueRef::Time64(_, _) => {
                                let d = DateTime::<Utc>::column_result(value).unwrap();
                                meta.push(MetaVariable {
                                    name: col.1.clone(),
                                    value: MetaValue::Date(d),
                                });
                            }
                            _ => {
                                error!(
                                    "Unexpected value type in meta column {}: {:?}",
                                    col.1, value
                                );
                            }
                        };
                        idx += 1;
                    }
                }

                if let Some(mappings) = MAPPINGS.get() {
                    apply_mappings(mappings, &mut meta);
                }

                fileindex::IndexFile {
                    filename,
                    url,
                    sha256,
                    created_at,
                    modified_at,
                    meta,
                }
            })
        });

        for indexfile in indexfile_iter? {
            callback(indexfile?);
        }
    }
    Ok(())
}
