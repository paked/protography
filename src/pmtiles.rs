use bytes::{Buf, Bytes};
use flate2::read::GzDecoder;

use core::panic;
use std::collections::HashMap;
use std::convert::TryFrom;
use std::hash::Hash;
use std::io::Read;
use std::str;

static EXPECTED_MAGIC: &str = "PMTiles";
const EXPECTED_VERSION: u8 = 3;

pub type Result<T> = std::result::Result<T, PmtilesError>;

#[derive(Debug)]
pub enum PmtilesError {
    InvalidMagic,
    InvalidVersion,
    InvalidValue,
    VarintOverflowError,
    TooHighZIndex,

    // Stdlib
    #[allow(dead_code)]
    IoError(std::io::Error),

    // External crates
    #[allow(dead_code)]
    MvtReaderError(mvt_reader::error::ParserError),
}

impl From<std::io::Error> for PmtilesError {
    fn from(value: std::io::Error) -> Self {
        PmtilesError::IoError(value)
    }
}

impl From<mvt_reader::error::ParserError> for PmtilesError {
    fn from(value: mvt_reader::error::ParserError) -> Self {
        PmtilesError::MvtReaderError(value)
    }
}

fn decompress_range(file: &[u8], start: usize, end: usize) -> Result<Vec<u8>> {
    let compressed_bytes = &file[start..end];

    let mut gz = GzDecoder::new(compressed_bytes);
    let mut bytes: Vec<u8> = Vec::new();
    gz.read_to_end(&mut bytes)?;

    Ok(bytes)
}

pub fn parse_directory(file: &[u8], start: usize, length: usize) -> Result<Vec<TileEntry>> {
    let bytes = decompress_range(file, start, start + length)?;
    let mut bytes = Bytes::from(bytes);

    let tile_num = parse_varint(&mut bytes)?;

    let mut tile_entries = vec![
        TileEntry {
            id: TileId(0),
            offset: 0,
            length: 0,
            run_length: 0
        };
        tile_num as usize
    ];

    let mut last_id = 0;
    for tile in tile_entries.iter_mut() {
        let id_delta = parse_varint(&mut bytes)?;
        last_id += id_delta;

        tile.id = TileId(last_id);
    }

    for tile in tile_entries.iter_mut() {
        let run_length = parse_varint(&mut bytes)?;

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

    Ok(tile_entries)
}

pub fn parse_root_directory(file: &[u8], header: &Header) -> Result<Vec<TileEntry>> {
    let entries = parse_directory(
        file,
        header.root_directory_offset as usize,
        header.root_directory_length as usize,
    )?;

    Ok(entries)
}

// PMTiles V3 Header.
#[derive(Debug)]
#[allow(dead_code)]
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
    pub min_position: Position,
    pub max_position: Position,
    pub center_zoom: u8,
    pub center_position: Position,
}

pub fn parse_header(bytes: &mut Bytes) -> Result<Header> {
    let magic = bytes.split_to(EXPECTED_MAGIC.len()).to_vec();
    let magic = str::from_utf8(&magic).unwrap();

    if magic != EXPECTED_MAGIC {
        return Err(PmtilesError::InvalidMagic);
    }

    let version = bytes.get_u8();
    if version != EXPECTED_VERSION {
        return Err(PmtilesError::InvalidVersion);
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
    type Error = PmtilesError;
    fn try_from(value: u8) -> std::result::Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::NotClustered),
            1 => Ok(Self::Clustered),
            _ => Err(PmtilesError::InvalidValue),
        }
    }
}

#[derive(Debug)]
enum TileType {
    Unknown,
    Mvt,
    Png,
    Jpeg,
    WebP,
    Avif,
}

impl TryFrom<u8> for TileType {
    type Error = PmtilesError;
    fn try_from(value: u8) -> std::result::Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Unknown),
            1 => Ok(Self::Mvt),
            2 => Ok(Self::Png),
            3 => Ok(Self::Jpeg),
            4 => Ok(Self::WebP),
            5 => Ok(Self::Avif),
            _ => Err(PmtilesError::InvalidValue),
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
    type Error = PmtilesError;
    fn try_from(value: u8) -> std::result::Result<Self, Self::Error> {
        match value {
            0x0 => Ok(Self::Unknown),
            0x1 => Ok(Self::None),
            0x2 => Ok(Self::GZip),
            0x3 => Ok(Self::Brotli),
            0x4 => Ok(Self::ZStd),
            _ => Err(PmtilesError::InvalidValue),
        }
    }
}

#[derive(Clone, Debug)]
pub struct TileEntry {
    pub id: TileId,
    pub offset: u64,
    pub length: u64,
    pub run_length: u64,
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

fn parse_varint(bytes: &mut bytes::Bytes) -> Result<u64> {
    let mut n: u64 = 0;

    for i in 0.. {
        let byte = bytes.get_u8();
        let value = (byte & !VARINT_CONTINUATION_BIT_MASK) as u64;
        n |= value
            .checked_shl(i * 7)
            .ok_or(PmtilesError::VarintOverflowError)?;

        if byte & VARINT_CONTINUATION_BIT_MASK == 0 {
            break;
        }
    }

    Ok(n)
}

#[derive(Copy, Clone, Debug)]
pub struct TileCoord {
    pub x: f64,
    pub y: f64,
    pub z: u8,
}

// TODO(harrisons): maybe should differentiate between a "perfect tile coordinate" and a "fractional one"?
impl TileCoord {
    // TODO: is there a better name for this
    pub fn ix(&self) -> u32 {
        self.x.floor() as u32
    }

    pub fn iy(&self) -> u32 {
        self.y.floor() as u32
    }
}

#[derive(PartialEq, PartialOrd, Eq, Hash, Clone, Copy, Debug)]
pub struct TileId(u64);

impl TryFrom<TileCoord> for TileId {
    type Error = PmtilesError;

    // implementation stolen/inspired by https://github.com/arma-place/pmtiles-rs, under MIT license
    fn try_from(value: TileCoord) -> std::result::Result<Self, Self::Error> {
        let TileCoord { z, .. } = value;
        if z > MAX_Z {
            return Err(PmtilesError::TooHighZIndex);
        }

        let x = value.ix();
        let y = value.iy();

        // FIXME: precompute this
        let base_id: u64 = 1 + (1..z).map(|i| 4u64.pow(u32::from(i))).sum::<u64>();

        let id = TileId(fast_hilbert::xy2h(x, y, z) + base_id);

        Ok(id)
    }
}

impl TryFrom<TileId> for TileCoord {
    type Error = PmtilesError;

    // implementation stolen/inspired by https://github.com/arma-place/pmtiles-rs, under MIT license
    fn try_from(id: TileId) -> std::result::Result<Self, Self::Error> {
        if id.0 == 0 {
            return Ok(TileCoord {
                x: 0.0,
                y: 0.0,
                z: 0,
            });
        }

        // TODO: pre-compute these base_id and z values
        let z = find_z(id.0)?;

        let base_id: u64 = 1 + (1..z).map(|i| 4u64.pow(u32::from(i))).sum::<u64>();

        let (x, y) = fast_hilbert::h2xy::<u32>(id.0 - base_id, z);

        Ok(TileCoord {
            x: x as f64,
            y: y as f64,
            z,
        })
    }
}

const MAX_Z: u8 = 32;

fn find_z(id: u64) -> std::result::Result<u8, PmtilesError> {
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
        return Err(PmtilesError::TooHighZIndex);
    }

    Ok(z)
}

// From chatgpt
pub fn lat_lon_to_xyz(lat: f64, lon: f64, zoom: u8) -> TileCoord {
    let lat_rad = lat.to_radians();
    let n = 2f64.powi(zoom as i32);

    let x = (lon + 180.0) / 360.0 * n;
    let y = (1.0 - (lat_rad.tan().asinh() / std::f64::consts::PI)) / 2.0 * n;

    TileCoord { x, y, z: zoom }
}

// From chatgpt
#[allow(dead_code)]
pub fn xyz_to_lat_lon(x: u32, y: u32, zoom: u8) -> Position {
    let n = 2f64.powi(zoom as i32);
    let lon = x as f64 / n * 360.0 - 180.0;

    let lat_rad = ((1.0 - 2.0 * (y as f64 / n)) * std::f64::consts::PI)
        .sinh()
        .atan();
    let lat = lat_rad.to_degrees();

    Position { lat, long: lon }
}

pub struct LeafDirectory {
    start: u64,
    entries: Vec<TileEntry>,
}

pub struct TileManager {
    data: Vec<u8>,
    pub header: Header,
    entries: Vec<TileEntry>,

    leaf_directories: Vec<LeafDirectory>,

    // TODO: this should probably be a smarter structure. Hashing a tile id is a bit redundant,
    //  since it's already a number. Should store tiles as "chunks", and index via their slippy coords
    //  which guarantees they'll be close in memory.
    loaded_tiles: HashMap<TileId, mvt_reader::Reader>,
}

impl TileManager {
    pub fn new(data: Vec<u8>) -> Result<Self> {
        let mut bytes = Bytes::from(data.clone());

        let header = parse_header(&mut bytes)?;

        let entries = parse_root_directory(&data, &header)?;

        Ok(TileManager {
            data,
            header,
            entries,
            loaded_tiles: HashMap::new(),
            leaf_directories: Vec::new(),
        })
    }

    fn tile_to_mvt_reader(&self, tile: &TileEntry) -> Result<mvt_reader::Reader> {
        let tile_data_start = (self.header.tile_data_offset + tile.offset) as usize;
        let tile_data_end = tile_data_start + tile.length as usize;
        let tile_data_bytes = decompress_range(&self.data, tile_data_start, tile_data_end)?;

        Ok(mvt_reader::Reader::new(tile_data_bytes)?)
    }

    fn find_or_insert_leaf_directory(&mut self, leaf: &TileEntry) -> &Vec<TileEntry> {
        assert!(
            leaf.run_length == 0,
            "function must only be called for leaf directories"
        );

        let pos = self
            .leaf_directories
            .iter()
            .position(|dir| dir.start == leaf.id.0);

        if let Some(dir) = pos {
            return &self.leaf_directories[dir].entries;
        }

        let start = self.header.leaf_directories_offset + leaf.offset;
        let entries =
            parse_directory(&self.data, start as usize, (start + leaf.length) as usize).unwrap();

        self.leaf_directories.push(LeafDirectory {
            start: leaf.id.0,
            entries,
        });

        // FIXME: remove unwrap
        self.leaf_directories
            .last()
            .map(|dir| &dir.entries)
            .unwrap()
    }

    fn find_tile(&mut self, id: TileId) -> Option<&TileEntry> {
        let mut found_idx = None;

        // TODO: binary search here, and handle run length
        for (i, entry) in self.entries.iter().enumerate() {
            if entry.id > id {
                break;
            }

            found_idx = Some(i);
        }

        if found_idx.is_none() {
            println!("no found idx?");
            panic!("hi");
        }

        let found_idx = found_idx?;

        if self.entries[found_idx].run_length == 0 {
            // TODO: split searching/inserting so we don't need to clone here
            let leaf_entries = self.find_or_insert_leaf_directory(&self.entries[found_idx].clone());

            // TODO: binary search here
            leaf_entries.iter().find(|leaf| {
                assert!(
                    leaf.run_length != 0,
                    "no support for recursive leaf directories"
                );

                id >= leaf.id && id < TileId(leaf.id.0 + leaf.run_length)
            })
        } else if self.entries[found_idx].id == id {
            Some(&self.entries[found_idx])
        } else {
            None
        }
    }

    pub fn get_tile(&mut self, id: TileId) -> Result<Option<&mvt_reader::Reader>> {
        if !self.loaded_tiles.contains_key(&id) {
            let tile_entry = match self.find_tile(id) {
                // TODO(remove clone): bad bad bad
                Some(t) => t.clone(),
                None => return Ok(None),
            };

            let tile = self.tile_to_mvt_reader(&tile_entry)?;
            self.loaded_tiles.insert(id, tile);
        }

        Ok(self.loaded_tiles.get(&id))
    }
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

        assert_eq!(tile_coord.ix(), 3702);
        assert_eq!(tile_coord.iy(), 2509);
        assert_eq!(tile_coord.z, 12);
    }

    #[test]
    fn test_tile_id_from_xyz() {
        let tile_coord = TileCoord {
            x: 3702.0,
            y: 2509.0,
            z: 12,
        };

        let tile_id = TileId::try_from(tile_coord).expect("Should be convertible");
        assert_eq!(tile_id.0, 18007234);
    }
}
