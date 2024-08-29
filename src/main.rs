#![warn(unused_extern_crates, unused_crate_dependencies)]

use clap::{Parser, ValueEnum};
use csv::Writer;
use dateparser;
use fileindex::IndexFile;
use indexdb::query_fileindex;
use log::{debug, error, warn};
use std::io;
use summarystats::SummaryStats;
use tokio_macros as _;
use walkdir::WalkDir;

mod datesummary;
mod fileindex;
mod indexdb;
mod jsonmeta;
mod processing;
mod summarystats;
mod valuecountsummary;
mod variablemapping;

fn default_extensions() -> Vec<String> {
    vec![
        String::from("jpg"),
        String::from("heic"),
        String::from("mov"),
        String::from("png"),
        String::from("raw"),
        String::from("tiff"),
        String::from("arw"),
        String::from("nef"),
        String::from("dng"),
    ]
}

#[derive(Parser, Debug)]
#[command(author, version, about)]
/// Index photo library
struct Args {
    /// Path to the library data folder
    #[arg(required = true, short('l'))]
    library: String,

    /// Limit concurrency
    #[arg(long, short('c'))]
    concurrency: Option<usize>,

    /// Action to perform
    #[arg(required = true)]
    action: Action,

    /// Path to photo file(s) location
    #[arg()]
    photo_location: Vec<String>,

    /// Limit number of results when listing
    #[arg(short('N'))]
    list_limit: Option<usize>,

    /// Url matching for list
    #[arg(short('u'))]
    list_url: Option<String>,

    /// SHA256 matching for list
    #[arg(short('s'))]
    list_sha: Option<String>,

    /// Minimum date for search
    #[arg(short('d'))]
    min_date: Option<String>,

    /// Maximum date for search
    #[arg(short('D'))]
    max_date: Option<String>,

    /// Command that produces JSON output to run for each file
    #[arg(long, default_value_t = String::from("exiftool -b -j -"))]
    meta_cmd: String,

    /// when running the json metadata command, should we try to merge with existing data?
    #[arg(long)]
    meta_merge: Option<bool>,

    /// Summary parameters
    #[arg(long)]
    summary_options: Option<String>,

    #[arg(long, default_values_t = default_extensions())]
    allowed_extensions: Vec<String>,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum, Debug)]
enum Action {
    /// Index files into the library
    Index,
    /// List matching files on the file system
    List,
    /// Show entries in database in CSV format
    Show,
    /// Summarize entries to terminal
    Summarize,
    /// List metadata columns available
    MetaColumns,
}

#[tokio::main]
async fn main() {
    env_logger::init();

    let args = Args::parse();
    debug!("{:?}", args);

    // Check if the library path exists and is a directory
    if !std::path::Path::new(&args.library).exists() {
        error!("Library path {} does not exist", args.library);
        panic!("Library path does not exist");
    }

    if !std::path::Path::new(&args.library).is_dir() {
        error!("Library path {} is not a directory", args.library);
        panic!("Library path is not a directory");
    }

    // Initialize the database connection with args.library + "/photocat.db"
    indexdb::init_connection(&args.library);

    let filenames: Option<String> = if args.photo_location.is_empty() {
        None
    } else {
        Some(args.photo_location.join(","))
    };

    let min_date = args.min_date.map(|date| dateparser::parse(&date).unwrap());
    let max_date = args.max_date.map(|date| dateparser::parse(&date).unwrap());

    if args.action == Action::Show {
        let mut wtr = Writer::from_writer(io::stdout());
        let meta_columns = indexdb::get_meta_columns();
        let mut columns: Vec<String> = vec![
            String::from("url"),
            String::from("filename"),
            String::from("sha256"),
            String::from("created_at"),
            String::from("modified_at"),
        ];
        columns.extend(meta_columns.unwrap().into_iter().map(|x| x.1));
        wtr.write_record(columns).unwrap();

        query_fileindex(
            &args.list_sha,
            &filenames,
            &args.list_url,
            &args.list_limit,
            &min_date,
            &max_date,
            |record: IndexFile| {
                let mut row: Vec<String> = vec![
                    record.url,
                    record.filename,
                    record.sha256,
                    record.created_at.to_string(),
                    record.modified_at.to_string(),
                ];
                row.extend(record.meta.into_iter().map(|x| x.value.to_string()));
                wtr.write_record(row).unwrap();
            },
        )
        .expect("Query to fileindex failed");
        wtr.flush().unwrap();
    } else if args.action == Action::Summarize {
        let mut summary: SummaryStats = SummaryStats::new(&args.summary_options);

        query_fileindex(
            &args.list_sha,
            &filenames,
            &args.list_url,
            &args.list_limit,
            &min_date,
            &max_date,
            |record: IndexFile| {
                summary.add(&record);
            },
        )
        .expect("Query to fileindex failed");

        println!("{}", summary);
    } else if args.action == Action::MetaColumns {
        let meta_columns = indexdb::get_meta_columns();
        match meta_columns {
            Ok(meta_columns) => {
                if meta_columns.len() > 0 {
                    for (_, name, ctype) in meta_columns {
                        println!("{}: {}", name, ctype)
                    }
                } else {
                    println!("No metadata columns are available.")
                }
            }
            Err(e) => println!("No metadata columns are available. {:?}", e),
        }
    } else if args.action == Action::Index || args.action == Action::List {
        // enumerate files specified in the photo location
        let files = args
            .photo_location
            .iter()
            .flat_map(|dir| WalkDir::new(dir))
            .into_iter()
            .filter_map(Result::ok)
            .filter(|entry| {
                entry.path().is_file() && {
                    let extension = entry.path().extension().and_then(|ext| ext.to_str());
                    match extension {
                        Some(ext) => args.allowed_extensions.contains(&ext.to_lowercase()),
                        None => false,
                    }
                }
            })
            .map(|x| String::from(x.path().to_str().unwrap()));

        async fn action_fun(entry: String, context: (Action, String, bool)) {
            let (action, meta_cmd, meta_merge) = context;
            debug!("Action on file: {:?}", entry);
            match action {
                Action::Index => {
                    // this needs to be run as a separate blocking thread so it runs in parallel
                    let result = tokio::task::spawn_blocking(move || {
                        indexdb::index_file(
                            entry,
                            if meta_cmd.is_empty() {
                                None
                            } else {
                                Some(meta_cmd)
                            },
                            meta_merge,
                        )
                    })
                    .await;
                    if let Err(err) = result {
                        error!("Error processing file: {:?}", err);
                    }
                }
                Action::List => {
                    println!("{}", entry);
                }
                _ => {}
            }
        }

        // Iterate list of files in parallel
        let action = args.action;
        let meta_cmd = args.meta_cmd;
        let meta_merge = args.meta_merge.unwrap_or(false);
        processing::consume_concurrently(
            files,
            action_fun,
            &(action, meta_cmd, meta_merge),
            true,
            None,
        )
        .await;
    }
}
