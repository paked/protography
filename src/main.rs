mod pmtiles;
use pmtiles::*;

use std::env;
use std::fs;

use bytes::Bytes;

fn main() {
    let args: Vec<String> = env::args().collect();
    let path = &args[1];

    println!("path {}", path);

    let file = fs::read(path).unwrap();
    let mut bytes = Bytes::from(file.clone());

    let header = parse_header(&mut bytes);
    let header = header.unwrap();

    let root_directory_entries = parse_root_directory(&file, &header).unwrap();

    let tile = &root_directory_entries[0];

    let tile_data_start = (header.tile_data_offset + tile.offset) as usize;
    let tile_data_end = tile_data_start + tile.length as usize;
    let tile_data_bytes = decompress_range(&file, tile_data_start, tile_data_end).unwrap();

    let tile_mvt = mvt_reader::Reader::new(tile_data_bytes).unwrap();
    // Get layer names
    let layer_names = tile_mvt.get_layer_names().unwrap();
    for name in layer_names {
        println!("Layer: {}", name);
    }

    println!("features: {:?}", tile_mvt.get_features(0).unwrap());
}
