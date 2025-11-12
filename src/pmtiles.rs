use bytes::{Buf, Bytes};
use flate2::read::GzDecoder;

use std::convert::TryFrom;
use std::io::Error;
use std::io::Read;
use std::str;
use std::str::Utf8Error;

static EXPECTED_MAGIC: &str = "PMTiles";
const EXPECTED_VERSION: u8 = 3;

#[derive(Debug)]
pub enum ParseError {
    InvalidMagic,
    InvalidVersion,
    InvalidUtf8(Utf8Error),
    InvalidValue,
    IoError(std::io::Error),
}

impl From<std::io::Error> for ParseError {
    fn from(value: std::io::Error) -> Self {
        ParseError::IoError(value)
    }
}

// TODO: make private
pub fn decompress_range(file: &Vec<u8>, start: usize, end: usize) -> Result<Vec<u8>, Error> {
    let compressed_bytes = &file[start..end];

    let mut gz = GzDecoder::new(compressed_bytes);
    let mut bytes: Vec<u8> = Vec::new();
    gz.read_to_end(&mut bytes)?;

    Ok(bytes)
}

pub fn parse_root_directory(file: &Vec<u8>, header: &Header) -> Result<TileEntries, ParseError> {
    let root_directory_start = header.root_directory_offset as usize;
    let root_directory_end = root_directory_start + header.root_directory_length as usize;
    let root_directory_bytes = decompress_range(&file, root_directory_start, root_directory_end)?;

    let mut bytes = Bytes::from(root_directory_bytes);

    let tile_num = parse_varint(&mut bytes);

    let mut tile_entries = vec![TileEntry::default(); tile_num as usize];

    let mut last_id = 0;
    for tile in tile_entries.iter_mut() {
        let id_delta = parse_varint(&mut bytes);
        last_id = last_id + id_delta;

        tile.id = last_id;
    }

    for tile in tile_entries.iter_mut() {
        let run_length = parse_varint(&mut bytes);

        tile.run_length = run_length;
    }

    for tile in tile_entries.iter_mut() {
        let length = parse_varint(&mut bytes);

        tile.length = length;
    }

    let mut last_offset = 0;
    let mut last_len = 0;

    for (i, tile) in tile_entries.iter_mut().enumerate() {
        let value = parse_varint(&mut bytes);

        if value == 0 && i > 0 {
            tile.offset = last_offset + last_len;
        } else {
            tile.offset = value - 1;
        }

        last_offset = tile.offset;
        last_len = tile.length;
    }

    Ok(TileEntries {
        entries: tile_entries,
    })
}

// PMTiles V3 Header.
#[derive(Debug)]
pub struct Header {
    pub root_directory_offset: u64,
    pub root_directory_length: u64,
    pub metadata_offset: u64,
    pub metadata_length: u64,
    pub leaf_directories_offset: u64,
    pub leaf_directories_length: u64,
    pub tile_data_offset: u64,
    pub tile_data_length: u64,
    pub number_of_addressed_tiles: u64,
    pub number_of_tile_entires: u64,
    pub number_of_tile_contents: u64,
    pub clustered: Clustered,
    pub internal_compression: Compression,
    pub tile_compression: Compression,
    pub tile_type: TileType,
    pub min_zoom: u8,
    pub max_zoom: u8,
    pub min_position: u64, // TODO: split into long/lat as described
    pub max_position: u64, // TODO: split into long/lat as described
    pub center_zoom: u8,
    pub center_position: u64, // TODO: split into long/lat as described
}

pub fn parse_header(bytes: &mut Bytes) -> Result<Header, ParseError> {
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
        internal_compression: Compression::try_from(bytes.get_u8())?,
        tile_compression: Compression::try_from(bytes.get_u8())?,
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

impl TryFrom<u8> for Compression {
    type Error = ParseError;
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x0 => Ok(Self::Unknown),
            0x1 => Ok(Self::None),
            0x2 => Ok(Self::GZip),
            0x3 => Ok(Self::Brotli),
            0x4 => Ok(Self::ZStd),
            _ => Err(ParseError::InvalidValue),
        }
    }
}

#[derive(Default, Clone, Debug)]
pub struct TileEntry {
    pub id: u64,
    pub offset: u64,
    pub length: u64,
    pub run_length: u64,
}

pub struct TileEntries {
    pub entries: Vec<TileEntry>,
}

impl TileEntries {
    pub fn find_tile(self: Self, id: u64) -> Option<TileEntry> {
        // TODO: this should be a binary search, and take into account the fact that leaf entries exist
        for tile in self.entries {
            if tile.id == id {
                return Some(tile);
            }
        }

        None
    }
}

#[derive(Debug)]
pub struct Position {
    lat: f64,
    long: f64,
}

const VARINT_CONTINUATION_BIT_MASK: u8 = 0b10000000;

fn parse_varint(bytes: &mut bytes::Bytes) -> u64 {
    let mut n: u64 = 0;

    let mut i = 0;

    // FIXME: throw error on overflow (and we should be able to read more than 7 bytes of input data)

    // continuation bits
    let mut next_byte = bytes.get_u8();
    while next_byte & VARINT_CONTINUATION_BIT_MASK != 0 && i < 7 {
        let byte = next_byte & !VARINT_CONTINUATION_BIT_MASK;
        n |= (byte as u64) << i * 7;

        next_byte = bytes.get_u8();
        i += 1;
    }

    // final bit
    n |= (next_byte as u64) << i * 7;

    n
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_varint() {
        let data: Vec<u8> = vec![0b10010110, 0b00000001];
        let mut bytes = Bytes::from(data);

        let n = parse_varint(&mut bytes);
        assert_eq!(n, 150);
    }

    #[test]
    fn test_gzip() {
        let bytes = fs::read("test.gz").unwrap();
        let mut gz = GzDecoder::new(&bytes[..]);
        let mut s = String::new();
        gz.read_to_string(&mut s).unwrap();

        let x = String::from("hello world\n");

        assert_eq!(s, x);
    }
}
