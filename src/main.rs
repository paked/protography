mod map_renderer;
mod pmtiles;
mod simple_vello;

use pmtiles::*;
use vello::util::RenderContext;
use winit::event_loop::EventLoop;

use std::env;
use std::fs;

use bytes::Bytes;

use crate::map_renderer::MapRenderer;

fn test_pmtiles() -> mvt_reader::Reader {
    let args: Vec<String> = env::args().collect();
    let path = &args[1];

    println!("path {}", path);

    let file = fs::read(path).unwrap();
    let mut bytes = Bytes::from(file.clone());

    let header = parse_header(&mut bytes);
    let header = header.unwrap();

    let root_directory_entries = parse_root_directory(&file, &header).unwrap();

    let tile = root_directory_entries.find_tile(18007234).unwrap();

    let tile_data_start = (header.tile_data_offset + tile.offset) as usize;
    let tile_data_end = tile_data_start + tile.length as usize;
    let tile_data_bytes = decompress_range(&file, tile_data_start, tile_data_end).unwrap();

    let tile_mvt = mvt_reader::Reader::new(tile_data_bytes).unwrap();
    // Get layer names
    let layer_names = tile_mvt.get_layer_names().unwrap();
    for name in layer_names {
        println!("Layer: {}", name);
    }

    tile_mvt
}

fn main() {
    println!("loading pmtiles data");
    let tile = test_pmtiles();
    println!("loaded pmtiles data");

    println!("setting up vello app");
    // Setup a bunch of state:
    let mut app = simple_vello::SimpleVelloApp {
        context: RenderContext::new(),
        renderers: vec![],
        state: simple_vello::RenderState::Suspended(None),
        scene: vello::Scene::new(),
        map_renderer: MapRenderer::new(tile),
    };
    println!("set up vello app");

    println!("starting event loop");

    // Create and run a winit event loop
    let event_loop = EventLoop::new().unwrap();
    event_loop
        .run_app(&mut app)
        .expect("Couldn't run event loop");
}
