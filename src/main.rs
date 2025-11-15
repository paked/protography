mod map_renderer;
mod pmtiles;
mod simple_vello;

use pmtiles::*;
use vello::util::RenderContext;
use winit::event_loop::EventLoop;

use std::env;
use std::fs;
use std::time::Instant;

use bytes::Bytes;

use crate::map_renderer::Camera;
use crate::map_renderer::MapRenderer;

fn test_pmtiles() -> mvt_reader::Reader {
    let args: Vec<String> = env::args().collect();
    let path = &args[1];

    let file = fs::read(path).unwrap();
    let mut bytes = Bytes::from(file.clone());

    let header = parse_header(&mut bytes);
    let header = header.unwrap();

    println!("{:#?}", header);

    let root_directory_entries = parse_root_directory(&file, &header).unwrap();

    let pos = &header.center_position;
    let zoom = 11;
    let coord = pmtiles::lat_lon_to_xyz(pos.lat, pos.long, zoom);
    let tile_id = TileId::try_from(coord).unwrap();

    let tile = root_directory_entries.find_tile(tile_id).unwrap();

    tile_to_mvt_reader(&header, tile, &file)
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
        camera: Camera {
            x: 0.0,
            y: 0.0,
            width: 1,
            height: 1,
        },
        last_frame_time: Instant::now(),
    };
    println!("set up vello app");

    println!("starting event loop");

    // Create and run a winit event loop
    let event_loop = EventLoop::new().unwrap();
    event_loop
        .run_app(&mut app)
        .expect("Couldn't run event loop");
}
