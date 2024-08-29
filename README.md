# Photocat - A photo metadata cataloging tool

Photocat is a small tool to collect and summarize and metadata in photos. Metadata are collected using
[exiftool](https://exiftool.org/) and organised & queried using [duckdb](https://duckdb.org/).

## How to use

By default, metadata are extracted using [exiftool](https://exiftool.org/). We keep a record for each file
 that identifies it by a sha256 hash of the file contents and its URL.

```bash
mkdir ./data
# index a folder and run exiftool on each image
photocat -l ./data index <folder-containing photos>
```

> [!NOTE]
> Custom metadata from any command specified by `--meta-cmd` can be added into a JSON file
> named as `<sha256>.json` in the data folder. Metadata get updated each time
> indexing is run.  JSON files are overwritten unless we specify `--meta-merge true`.
> The default metadata command is `exiftool -j -b -` - the metadata command receives the
> image by piping from stdin.

We can summarize from the content of the database as follows, displaying when files were
created Jun-Aug 2024:

```bash
photocat -l ./data summarize -d 2024-06-01 -D 2024-09-01
```

```text
#total:128
2024 ┌JUL 
 Sun  ░░▉░░░░░  
 Mon ░░░▒▓░░░░  
 Tue ░▒░░░░░░░  
 Wed ░░▒░░░░░░  
 Thu ░░░▒░░░░░  
 Fri ░▒▓▉▓░░░░  
 Sat ░▒▉▓▉░░░░  
```

> [!NOTE]
> This summary by default will use the DateTimeOriginal EXIF field, but when this isn't present
> the creation time of the file will be used. When this happens, the summary will list for how
> many files no exif data was available.

## EXIF Metadata Collection

When running EXIFTool as part of the indexing step like shown above, we create a set of JSON
files in the data directory. These JSON files are used to retrieve select EXIF metadata into
a form that can be queried by the catalog tool. The variables are listed in
[src/meta.sql](conf/meta.sql) and can be configured at compile time by changing that SQL script.

All collected metadata can be queried as CSV:

```bash
photocat -l ./data show -d 2024-01-01
```

```text
url,filename,sha256,created_at,modified_at,sha256,Lens,LensInfo,LensModel,Make,Model,Aperture,ShutterSpeed,ISO,ImageWidth,ImageHeight,Software,DateTaken
file:///<...>/DSC_5632.jpg,/<...>/DSC_5632.jpg,73a561fbe307be4578b2742af3c97e0663140ae748fdd62aa275b97f04ebe8aa,2024-02-03 ...
...
```

It is also possible to summarize the values of one or more metadata variables in a table:

```bash
photocat -l ./data summarize -d 2024-07-1 --summary-options count:Lens
```

```text
...
╭───────────────────────────────────┬───────╮
│ Lens                              ┆ Count │
╞═══════════════════════════════════╪═══════╡
│ 85mm f/1.8                        ┆ 52    │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌┤
│ 35mm f/1.8                        ┆ 36    │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌┤
│ 15mm f/4.5                        ┆ 2     │
╰───────────────────────────────────┴───────╯
```

## Remapping / cleaning metadata

Sometimes different processing software changes EXIF names of lenses or camera models. We can fix this in
 our summaries using a file named [mapping.toml](data/mapping.toml) in our database folder. This file
 can contain mappings of values as follows:

```toml
[[mapping]]
variable = 'Lens'
match_values = ['15 mm f/4.5']
assign_value = '15mm f/4.5'
```

This mapping shortens *15 mm f/4.5* to *15mm f/4.5* in the Lens variable. Multiple
values can be mapped to one in this way, and multiple mappings for different variables are allowed. Note that
non-string values can be replaced but the resulting value will be a string internally.

## Development notes

Photocat is a command line program written in Rust.

Setting up a development environment:

```bash
git clone https://github.com/pkrusche/photocat.git
cd photocat
cargo build
```

Running the tests:

```bash
cargo test
```

Code checks ensure that code is formatted using:

```bash
cargo fmt
```
