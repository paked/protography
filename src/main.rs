mod app;
mod error;
mod map_renderer;
mod pmtiles;

use error::Result;
use pmtiles::*;
use vello::util::RenderContext;
use winit::event_loop::EventLoop;

use std::env;
use std::fs;
use std::time::Instant;

use crate::app::Input;
use crate::map_renderer::Camera;
use crate::map_renderer::MapRenderer;

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    let path = &args[1];
    let file = fs::read(path).unwrap();

    let tile_manager = TileManager::new(file).unwrap();

    println!("setting up vello app");

    let world_origin = Position {
        ..tile_manager.header.center_position
    };

    // Setup a bunch of state:
    let mut app = app::App {
        context: RenderContext::new(),
        renderers: vec![],
        state: app::RenderState::Suspended(None),
        scene: vello::Scene::new(),
        map_renderer: MapRenderer::new(),
        tile_manager,
        camera: Camera {
            x: 0.0,
            y: 0.0,
            zoom: 1.0,
            world_origin,
            width: 1,
            height: 1,
        },
        input: Input::default(),
        last_frame_time: Instant::now(),
    };

    println!("starting event loop");

    // Create and run a winit event loop
    let event_loop = EventLoop::new().unwrap();
    event_loop
        .run_app(&mut app)
        .expect("Couldn't run event loop");

    Ok(())
}
