use bytes::{Buf, Bytes};
use flate2::read::GzDecoder;

use std::convert::TryFrom;
use std::io::Error;
use std::io::Read;
use std::str;
use std::str::Utf8Error;

static EXPECTED_MAGIC: &str = "PMTiles";
const EXPECTED_VERSION: u8 = 3;

pub fn tile_to_mvt_reader(header: &Header, tile: &TileEntry, file: &Vec<u8>) -> mvt_reader::Reader {
    let tile_data_start = (header.tile_data_offset + tile.offset) as usize;
    let tile_data_end = tile_data_start + tile.length as usize;
    let tile_data_bytes = decompress_range(file, tile_data_start, tile_data_end).unwrap();

    mvt_reader::Reader::new(tile_data_bytes).unwrap()
}

#[derive(Debug)]
pub enum ParseError {
    InvalidMagic,
    InvalidVersion,
    InvalidUtf8(Utf8Error),
    InvalidValue,
    IoError(std::io::Error),
    VarintOverflowError,
    TooHighZIndex,
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

    let tile_num = parse_varint(&mut bytes)?;

    let mut tile_entries = vec![TileEntry::default(); tile_num as usize];

    let mut last_id = 0;
    for tile in tile_entries.iter_mut() {
        let id_delta = parse_varint(&mut bytes)?;
        last_id = last_id + id_delta;

        tile.id = last_id;
    }

    for tile in tile_entries.iter_mut() {
        let run_length = parse_varint(&mut bytes)?;

        // FIXME
        assert!(
            run_length != 0,
            "Run length 0 indicates a leaf entry, which is not implemented"
        );

        tile.run_length = run_length;
    }

    for tile in tile_entries.iter_mut() {
        let length = parse_varint(&mut bytes)?;

        tile.length = length;
    }

    let mut last_offset = 0;
    let mut last_len = 0;

    for (i, tile) in tile_entries.iter_mut().enumerate() {
        let value = parse_varint(&mut bytes)?;

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
    root_directory_offset: u64,
    root_directory_length: u64,
    metadata_offset: u64,
    metadata_length: u64,
    leaf_directories_offset: u64,
    leaf_directories_length: u64,
    pub tile_data_offset: u64,
    pub tile_data_length: u64,
    number_of_addressed_tiles: u64,
    number_of_tile_entires: u64,
    number_of_tile_contents: u64,
    clustered: Clustered,
    internal_compression: Compression,
    tile_compression: Compression,
    tile_type: TileType,
    min_zoom: u8,
    max_zoom: u8,
    min_position: Position,
    max_position: Position,
    pub center_zoom: u8,
    pub center_position: Position,
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
        min_position: Position::from(bytes.get_u64_le()),
        max_position: Position::from(bytes.get_u64_le()),
        center_zoom: bytes.get_u8(),
        center_position: Position::from(bytes.get_u64_le()),
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
    pub fn find_tile(&self, id: TileId) -> Option<&TileEntry> {
        self.entries.iter().find(|e| e.id == id.0)
    }
}

#[derive(Debug)]
pub struct Position {
    pub lat: f64,
    pub long: f64,
}

impl From<u64> for Position {
    fn from(value: u64) -> Self {
        let long = (value & 0xFFFF_FFFF) as i32;
        let lat = (value >> 32) as i32;

        let long = (long as f64) / 10_000_000.0;
        let lat = (lat as f64) / 10_000_000.0;

        Position { long, lat }
    }
}

const VARINT_CONTINUATION_BIT_MASK: u8 = 0b10000000;

fn parse_varint(bytes: &mut bytes::Bytes) -> Result<u64, ParseError> {
    let mut n: u64 = 0;

    for i in 0.. {
        let byte = bytes.get_u8();
        let value = (byte & !VARINT_CONTINUATION_BIT_MASK) as u64;
        n |= value
            .checked_shl(i * 7)
            .ok_or(ParseError::VarintOverflowError)?;

        if byte & VARINT_CONTINUATION_BIT_MASK == 0 {
            break;
        }
    }

    Ok(n)
}

pub struct TileCoord {
    x: u32,
    y: u32,
    z: u8,
}

pub struct TileId(u64);

impl TryFrom<TileCoord> for TileId {
    type Error = ParseError;

    // implementation stolen/inspired by https://github.com/arma-place/pmtiles-rs, under MIT license
    fn try_from(value: TileCoord) -> Result<Self, Self::Error> {
        let TileCoord { x, y, z } = value;
        if z > MAX_Z {
            return Err(ParseError::TooHighZIndex);
        }

        // FIXME: precompute this
        let base_id: u64 = 1 + (1..z).map(|i| 4u64.pow(u32::from(i))).sum::<u64>();

        // FIXME: should x, y just be u32?
        let id = TileId(fast_hilbert::xy2h(x as u32, y as u32, z) + base_id);

        Ok(id)
    }
}

impl TryFrom<TileId> for TileCoord {
    type Error = ParseError;

    // implementation stolen/inspired by https://github.com/arma-place/pmtiles-rs, under MIT license
    fn try_from(id: TileId) -> Result<Self, Self::Error> {
        if id.0 == 0 {
            return Ok(TileCoord { x: 0, y: 0, z: 0 });
        }

        // TODO: pre-compute these base_id and z values

        let z = find_z(id.0)?;

        let base_id: u64 = 1 + (1..z).map(|i| 4u64.pow(u32::from(i))).sum::<u64>();

        let (x, y) = fast_hilbert::h2xy::<u32>(id.0 - base_id, z);

        Ok(TileCoord { x, y, z })
    }
}

const MAX_Z: u8 = 32;

fn find_z(id: u64) -> Result<u8, ParseError> {
    let mut z: u8 = 0;
    let mut acc: u64 = 1;

    for i in 1u8..MAX_Z {
        let num_tiles = 4_u64.pow(u32::from(i));
        acc += num_tiles;

        if acc > id {
            z = i;
            break;
        }
    }

    if z == 0 {
        return Err(ParseError::TooHighZIndex);
    }

    Ok(z)
}

// From chatgpt
pub fn lat_lon_to_xyz(lat: f64, lon: f64, zoom: u8) -> TileCoord {
    let lat_rad = lat.to_radians();
    let n = 2f64.powi(zoom as i32);

    let x = ((lon + 180.0) / 360.0 * n).floor() as u32;
    let y = ((1.0 - (lat_rad.tan().asinh() / std::f64::consts::PI)) / 2.0 * n).floor() as u32;

    TileCoord { x, y, z: zoom }
}

// From chatgpt
pub fn xyz_to_lat_lon(x: u32, y: u32, zoom: u8) -> Position {
    let n = 2f64.powi(zoom as i32);
    let lon = x as f64 / n * 360.0 - 180.0;

    let lat_rad = ((1.0 - 2.0 * (y as f64 / n)) * std::f64::consts::PI)
        .sinh()
        .atan();
    let lat = lat_rad.to_degrees();

    Position { lat, long: lon }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_varint_1() {
        let data: Vec<u8> = vec![0b10010110, 0b00000001];
        let mut bytes = Bytes::from(data);

        let n = parse_varint(&mut bytes).expect("Should parse value");
        assert_eq!(n, 150);
    }

    #[test]
    fn test_parse_varint_2() {
        // this is too much data to fit in a u64, which is what we're turning our varints into.
        let data: Vec<u8> = vec![
            0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x00,
        ];
        let mut bytes = Bytes::from(data);

        let n = parse_varint(&mut bytes);
        assert!(n.is_err());
    }

    #[test]
    fn test_gzip() {
        let bytes = std::fs::read("test.txt.gz").unwrap();
        let mut gz = GzDecoder::new(&bytes[..]);
        let mut s = String::new();
        gz.read_to_string(&mut s).unwrap();

        let x = String::from("hello world\n");

        assert_eq!(s, x);
    }

    #[test]
    fn test_tile_xyz_from_id() {
        let tile_coord = TileCoord::try_from(TileId(18007234)).expect("Should be convertible");

        assert_eq!(tile_coord.x, 3702);
        assert_eq!(tile_coord.y, 2509);
        assert_eq!(tile_coord.z, 12);
    }

    #[test]
    fn test_tile_id_from_xyz() {
        let tile_coord = TileCoord {
            x: 3702,
            y: 2509,
            z: 12,
        };

        let tile_id = TileId::try_from(tile_coord).expect("Should be convertible");
        assert_eq!(tile_id.0, 18007234);
    }
}
