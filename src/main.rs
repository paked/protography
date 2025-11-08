use bytes::{Buf, Bytes};
use std::convert::TryFrom;
use std::env;
use std::fs;
use std::str;
use std::str::Utf8Error;

static EXPECTED_MAGIC: &str = "PMTiles";
const EXPECTED_VERSION: u8 = 3;

#[derive(Debug)]
enum ParseError {
    InvalidMagic,
    InvalidVersion,
    InvalidUtf8(Utf8Error),
    InvalidValue,
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let path = &args[1];

    println!("path {}", path);

    let file = fs::read(path).unwrap();
    let mut bytes = Bytes::from(file);

    let header = parse_header(&mut bytes);
    let header = header.unwrap();

    println!("got header: {:#?}", header);
}

// PMTiles V3 Header.
#[derive(Debug)]
struct Header {
    root_directory_offset: u64,
    root_directory_length: u64,
    metadata_offset: u64,
    metadata_length: u64,
    leaf_directories_offset: u64,
    leaf_directories_length: u64,
    tile_data_offset: u64,
    tile_data_length: u64,
    number_of_addressed_tiles: u64,
    number_of_tile_entires: u64,
    number_of_tile_contents: u64,
    clustered: Clustered,
    internal_compression: u8, // TODO: convert to enum
    tile_compression: u8,     // TODO: convert to enum,
    tile_type: TileType,
    min_zoom: u8,
    max_zoom: u8,
    min_position: u64, // TODO: split into long/lat as described
    max_position: u64, // TODO: split into long/lat as described
    center_zoom: u8,
    center_position: u64, // TODO: split into long/lat as described
}

fn parse_header(bytes: &mut Bytes) -> Result<Header, ParseError> {
    let magic = bytes.split_to(EXPECTED_MAGIC.len()).to_vec();
    let magic = str::from_utf8(&magic).unwrap();

    if magic != EXPECTED_MAGIC {
        return Err(ParseError::InvalidMagic);
    }

    let version = bytes.get_u8();
    if version != EXPECTED_VERSION {
        return Err(ParseError::InvalidVersion);
    }

    let header = Header {
        root_directory_offset: bytes.get_u64_le(),
        root_directory_length: bytes.get_u64_le(),
        metadata_offset: bytes.get_u64_le(),
        metadata_length: bytes.get_u64_le(),
        leaf_directories_offset: bytes.get_u64_le(),
        leaf_directories_length: bytes.get_u64_le(),
        tile_data_offset: bytes.get_u64_le(),
        tile_data_length: bytes.get_u64_le(),
        number_of_addressed_tiles: bytes.get_u64_le(),
        number_of_tile_entires: bytes.get_u64_le(),
        number_of_tile_contents: bytes.get_u64_le(),
        clustered: Clustered::try_from(bytes.get_u8())?,
        internal_compression: bytes.get_u8(),
        tile_compression: bytes.get_u8(),
        tile_type: TileType::try_from(bytes.get_u8())?,
        min_zoom: bytes.get_u8(),
        max_zoom: bytes.get_u8(),
        min_position: bytes.get_u64_le(),
        max_position: bytes.get_u64_le(),
        center_zoom: bytes.get_u8(),
        center_position: bytes.get_u64_le(),
    };

    Ok(header)
}

#[derive(Debug)]
enum Clustered {
    NotClustered,
    Clustered,
}

impl TryFrom<u8> for Clustered {
    type Error = ParseError;
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::NotClustered),
            1 => Ok(Self::Clustered),
            _ => Err(ParseError::InvalidValue),
        }
    }
}

#[derive(Debug)]
enum TileType {
    Unknown,
    MVT,
    PNG,
    JPEG,
    WebP,
    AVIF,
}

impl TryFrom<u8> for TileType {
    type Error = ParseError;
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Unknown),
            1 => Ok(Self::MVT),
            2 => Ok(Self::PNG),
            3 => Ok(Self::JPEG),
            4 => Ok(Self::WebP),
            5 => Ok(Self::AVIF),
            _ => Err(ParseError::InvalidValue),
        }
    }
}

#[derive(Debug)]
enum Compression {
    Unknown,
    None,
    GZip,
    Brotli,
    ZStd,
}

#[derive(Debug)]
struct Position {}
