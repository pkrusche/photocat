-- Included into indexdb.rs
-- Extract some commonly interesting EXIF values into a temporary table with a primary key
CREATE TEMP TABLE meta (
    sha256 VARCHAR PRIMARY KEY,
    Lens VARCHAR,
    LensInfo VARCHAR,
    LensModel VARCHAR,
    Make VARCHAR,
    Model VARCHAR,
    Aperture VARCHAR,
    ShutterSpeed VARCHAR,
    ISO VARCHAR,
    ImageWidth INT,
    ImageHeight INT,
    Software VARCHAR,
    FocalLength VARCHAR,
    FocalLengthIn35mmFormat VARCHAR,
    DateTaken TIMESTAMP,
    LensInferred VARCHAR
);

-- Insert data into the temporary table
INSERT INTO meta
SELECT sha256,
       Lens,
       LensInfo,
       LensModel,
       Make,
       Model,
       Aperture,
       ShutterSpeed,
       ISO,
       ImageWidth,
       ImageHeight,
       Software,
       FocalLength,
       FocalLengthIn35mmFormat,
       strptime(COALESCE(CreateDate, DateTimeOriginal),
           ['%Y:%m:%d %H:%M:%S.%f', -- Format with milliseconds
            '%Y:%m:%d %H:%M:%S']    -- Format without milliseconds
       ) AS DateTaken,
       COALESCE(
        Lens,
        CASE
            WHEN Model ILIKE '%iPhone%' THEN Model
            ELSE Lens
        END 
       ) AS LensInferred
FROM '*.json';

-- Create an index for the sha256 column
CREATE INDEX sha256_index ON meta (sha256);