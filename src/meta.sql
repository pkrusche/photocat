-- Included into indexdb.rs

-- Drop the view if it exists
DROP TABLE IF EXISTS meta;

-- Create a table to store the extracted EXIF values
CREATE TEMP TABLE meta (
    sha256 TEXT PRIMARY KEY,
    Artist TEXT,
    Lens TEXT,
    LensInfo TEXT,
    LensModel TEXT,
    Make TEXT,
    Model TEXT,
    Aperture TEXT,
    ShutterSpeed TEXT,
    ISO TEXT,
    ImageWidth INTEGER,
    ImageHeight INTEGER,
    Orientation TEXT,
    Software TEXT,
    FocalLength TEXT,
    FocalLengthIn35mmFormat TEXT,
    DateTakenStr TEXT,
    DateTaken TIMESTAMP,
    LensInferred TEXT
);

-- Insert data into the meta table
INSERT INTO meta
SELECT
    sha256,
    COALESCE(json_extract(read_json_auto, '$.Artist'), NULL) AS Artist,
    COALESCE(json_extract(read_json_auto, '$.Lens'), NULL) AS Lens,
    COALESCE(json_extract(read_json_auto, '$.LensInfo'), NULL) AS LensInfo,
    COALESCE(json_extract(read_json_auto, '$.LensModel'), NULL) AS LensModel,
    COALESCE(json_extract(read_json_auto, '$.Make'), NULL) AS Make,
    COALESCE(json_extract(read_json_auto, '$.Model'), NULL) AS Model,
    COALESCE(json_extract(read_json_auto, '$.Aperture'), NULL) AS Aperture,
    COALESCE(json_extract(read_json_auto, '$.ShutterSpeed'), NULL) AS ShutterSpeed,
    COALESCE(json_extract(read_json_auto, '$.ISO'), NULL) AS ISO,
    COALESCE(json_extract(read_json_auto, '$.ImageWidth'), 0) AS ImageWidth,
    COALESCE(json_extract(read_json_auto, '$.ImageHeight'), 0) AS ImageHeight,
    COALESCE(json_extract(read_json_auto, '$.Orientation'), NULL) AS Orientation,
    COALESCE(json_extract(read_json_auto, '$.Software'), NULL) AS Software,
    COALESCE(json_extract(read_json_auto, '$.FocalLength'), NULL) AS FocalLength,
    COALESCE(json_extract(read_json_auto, '$.FocalLengthIn35mmFormat'), NULL) AS FocalLengthIn35mmFormat,
    COALESCE(
        read_json_auto -> '$.CreateDate',
        read_json_auto -> '$.DateTimeOriginal',
        read_json_auto -> '$.Metadatadate',
        NULL
    ) AS DateTakenStr,
    COALESCE(
        try_strptime(DateTakenStr, '"%Y:%m:%d %H:%M:%S.%f"'),
        try_strptime(DateTakenStr, '"%Y:%m:%d %H:%M:%S"'),
        NULL
    ) AS DateTaken,
    COALESCE(
     json_extract(read_json_auto, '$.Lens'),
     CASE
         WHEN json_extract(read_json_auto, '$.Model') ILIKE '%iPhone%' THEN json_extract(read_json_auto, '$.Model')
         ELSE NULL
     END 
    ) AS LensInferred
FROM read_json_auto('{{datapath}}/*.json', ignore_errors=true, union_by_name=true);

-- Create an index for the sha256 column
CREATE INDEX sha256_index ON meta (sha256);
