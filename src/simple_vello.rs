// Based off of the "simple" example from vello.
// TODO: remove unwraps

use std::sync::Arc;
use std::time::Instant;
use vello::kurbo::{Affine, Circle, Stroke};
use vello::peniko::Color;
use vello::peniko::color::palette;
use vello::util::{RenderContext, RenderSurface};
use vello::{AaConfig, Renderer, RendererOptions, Scene};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{Key, NamedKey};
use winit::window::Window;

use vello::wgpu;

use crate::map_renderer::{Camera, MapRenderer};
use crate::pmtiles::{TileCoord, TileId, TileManager};

#[derive(Debug)]
pub enum RenderState {
    /// `RenderSurface` and `Window` for active rendering.
    Active {
        surface: Box<RenderSurface<'static>>,
        valid_surface: bool,
        window: Arc<Window>,
    },
    /// Cache a window so that it can be reused when the app is resumed after being suspended.
    Suspended(Option<Arc<Window>>),
}

pub struct SimpleVelloApp {
    /// The Vello `RenderContext` which is a global context that lasts for the
    /// lifetime of the application
    pub context: RenderContext,

    /// An array of renderers, one per wgpu device
    pub renderers: Vec<Option<Renderer>>,

    /// State for our example where we store the winit Window and the wgpu Surface
    pub state: RenderState,

    /// A vello Scene which is a data structure which allows one to build up a
    /// description a scene to be drawn (with paths, fills, images, text, etc)
    /// which is then passed to a renderer for rendering
    pub scene: Scene,

    pub input: Input,

    pub map_renderer: MapRenderer,
    pub tile_manager: TileManager,

    pub camera: Camera,

    pub last_frame_time: Instant,
}

impl ApplicationHandler for SimpleVelloApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let RenderState::Suspended(cached_window) = &mut self.state else {
            return;
        };

        // Get the winit window cached in a previous Suspended event or else create a new window
        let window = cached_window
            .take()
            .unwrap_or_else(|| create_winit_window(event_loop));

        window.focus_window();

        // Create a vello Surface
        let size = window.inner_size();
        let surface_future = self.context.create_surface(
            window.clone(),
            size.width,
            size.height,
            wgpu::PresentMode::AutoVsync,
        );
        // FIXME: do we need this one-off dependency on pollster?
        let surface = pollster::block_on(surface_future).expect("Error creating surface");

        // Create a vello Renderer for the surface (using its device id)
        self.renderers
            .resize_with(self.context.devices.len(), || None);
        self.renderers[surface.dev_id]
            .get_or_insert_with(|| create_vello_renderer(&self.context, &surface));

        // Save the Window and Surface to a state variable
        self.state = RenderState::Active {
            surface: Box::new(surface),
            valid_surface: true,
            window,
        };
    }

    fn suspended(&mut self, _event_loop: &ActiveEventLoop) {
        if let RenderState::Active { window, .. } = &self.state {
            self.state = RenderState::Suspended(Some(window.clone()));
        }
    }

    fn device_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _device_id: winit::event::DeviceId,
        event: winit::event::DeviceEvent,
    ) {
        match event {
            winit::event::DeviceEvent::MouseMotion { delta } => {
                self.input.mouse_dx = delta.0;
                self.input.mouse_dy = delta.1;
            }
            winit::event::DeviceEvent::MouseWheel {
                delta: winit::event::MouseScrollDelta::PixelDelta(delta),
            } => {
                self.input.mouse_wheel_dx = delta.x;
                self.input.mouse_wheel_dy = delta.y;
            }
            _ => (),
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        // Only process events for our window, and only when we have a surface.
        let (surface, valid_surface, window) = match &mut self.state {
            RenderState::Active {
                surface,
                valid_surface,
                window,
            } if window.id() == window_id => (surface, valid_surface, window),
            _ => return,
        };

        match event {
            // Exit the event loop when a close is requested (e.g. window's close button is pressed)
            WindowEvent::CloseRequested => event_loop.exit(),

            // Resize the surface when the window is resized
            WindowEvent::Resized(size) => {
                if size.width != 0 && size.height != 0 {
                    self.context
                        .resize_surface(surface, size.width, size.height);
                    *valid_surface = true;
                } else {
                    *valid_surface = false;
                }
            }

            WindowEvent::MouseInput {
                device_id: _device_id,
                state,
                button,
            } => match button {
                winit::event::MouseButton::Left => {
                    self.input.is_primary_pressed = state.is_pressed()
                }
                winit::event::MouseButton::Right => {
                    self.input.is_secondary_pressed = state.is_pressed()
                }
                _ => (),
            },

            WindowEvent::KeyboardInput { event, .. } => {
                if let Key::Named(key) = event.logical_key {
                    match key {
                        NamedKey::Space => {
                            self.input.is_space_pressed = event.state.is_pressed() && !event.repeat
                        }
                        NamedKey::Shift => {
                            self.input.is_shift_pressed = event.state.is_pressed() && !event.repeat
                        }
                        _ => (),
                    }
                }
            }

            // This is where all the rendering happens
            WindowEvent::RedrawRequested => {
                if !*valid_surface {
                    return;
                }

                window.request_redraw();

                let current_frame_time = Instant::now();
                let delta_time = current_frame_time
                    .duration_since(self.last_frame_time)
                    .as_secs_f64();
                self.last_frame_time = current_frame_time;

                // Empty the scene of objects to draw. You could create a new Scene each time, but in this case
                // the same Scene is reused so that the underlying memory allocation can also be reused.
                self.scene.reset();

                if self.input.is_primary_pressed {
                    self.camera.x +=
                        -self.input.mouse_dx * window.scale_factor() / self.camera.zoom;
                    self.camera.y +=
                        -self.input.mouse_dy * window.scale_factor() / self.camera.zoom;

                    self.input.mouse_dx = 0.0;
                    self.input.mouse_dy = 0.0;
                }

                if self.input.mouse_wheel_dy != 0.0 {
                    let coefficient = if self.input.mouse_wheel_dy > 0.0 {
                        1.01
                    } else {
                        0.99
                    };
                    self.camera.zoom *= coefficient;

                    self.input.mouse_wheel_dy = 0.0;
                }

                if self.input.is_space_pressed {
                    self.camera.zoom += 0.0005;

                    self.input.is_space_pressed = false;
                }

                println!("fps: {}, zoom: {}", 1.0 / delta_time, self.camera.zoom);

                println!(
                    "tile size world pixels {}",
                    self.camera.get_tile_size_in_world_pixels()
                );

                println!("tile size range {:?}", self.camera.get_tile_range());

                self.camera.width = surface.config.width;
                self.camera.height = surface.config.height;

                let (min_tile, max_tile) = self.camera.get_tile_range();

                let transform = Affine::translate((-self.camera.x, -self.camera.y));
                let transform = transform * Affine::scale(self.camera.get_tile_size_multipler());

                let world_origin = self.camera.world_origin();

                let zoom = Affine::scale(self.camera.zoom);

                let tile_size = self.camera.get_tile_size_in_world_pixels();

                println!(
                    "x: {:?} y: {:?} z: {}",
                    self.camera.x,
                    self.camera.y,
                    self.camera.get_slippy_zoom()
                );
                println!(
                    "tile size mulitplier {}",
                    self.camera.get_tile_size_multipler()
                );

                for x in min_tile.0..max_tile.0 {
                    for y in min_tile.1..max_tile.1 {
                        // fixme: remove unwraps
                        let tile_coord = TileCoord {
                            x: x as f64,
                            y: y as f64,
                            z: world_origin.z,
                        };
                        let tile_id = TileId::try_from(tile_coord).unwrap();
                        let tile = self.tile_manager.get_tile(tile_id).unwrap();

                        let tile = match tile {
                            Some(t) => t,
                            None => {
                                continue;
                            }
                        };

                        let x = x as f64;
                        let y = y as f64;
                        let world_origin_x = world_origin.x;
                        let world_origin_y = world_origin.y;

                        let tile_x = x - world_origin_x;
                        let tile_y = y - world_origin_y;

                        let screen_x = tile_x * tile_size;
                        let screen_y = tile_y * tile_size;

                        let transform = Affine::translate((screen_x, screen_y)) * transform;
                        let transform = zoom * transform;

                        let transform = Affine::translate((
                            (self.camera.width / 2) as f64,
                            (self.camera.height / 2) as f64,
                        )) * transform;

                        self.map_renderer
                            .render_to_scene(tile, &mut self.scene, transform);
                    }
                }

                let origin_transform = Affine::translate((0.0, 0.0)) * transform;
                let origin_transform = zoom * origin_transform;
                let origin_transform = Affine::translate((
                    (self.camera.width / 2) as f64,
                    (self.camera.height / 2) as f64,
                )) * origin_transform;

                let my_stroke = Stroke::new(6.0);
                let my_color = Color::new([1.0, 1.0, 1.0, 1.0]);
                self.scene.stroke(
                    &my_stroke,
                    origin_transform,
                    my_color,
                    None,
                    &Circle::new((0.0, 0.0), 10.0),
                );

                // Get a handle to the device
                let device_handle = &self.context.devices[surface.dev_id];

                // Render to a texture, which we will later copy into the surface
                self.renderers[surface.dev_id]
                    .as_mut()
                    .unwrap()
                    .render_to_texture(
                        &device_handle.device,
                        &device_handle.queue,
                        &self.scene,
                        &surface.target_view,
                        &vello::RenderParams {
                            base_color: palette::css::BLACK, // Background color
                            width: self.camera.width,
                            height: self.camera.height,
                            antialiasing_method: AaConfig::Msaa16,
                        },
                    )
                    .expect("failed to render to surface");

                // Get the surface's texture
                let surface_texture = surface
                    .surface
                    .get_current_texture()
                    .expect("failed to get surface texture");

                // Perform the copy
                let mut encoder =
                    device_handle
                        .device
                        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                            label: Some("Surface Blit"),
                        });
                surface.blitter.copy(
                    &device_handle.device,
                    &mut encoder,
                    &surface.target_view,
                    &surface_texture
                        .texture
                        .create_view(&wgpu::TextureViewDescriptor::default()),
                );
                device_handle.queue.submit([encoder.finish()]);
                // Queue the texture to be presented on the surface
                surface_texture.present();

                device_handle.device.poll(wgpu::PollType::Poll).unwrap();
            }
            _ => {}
        }
    }
}

/// Helper function that creates a Winit window and returns it (wrapped in an Arc for sharing between threads)
fn create_winit_window(event_loop: &ActiveEventLoop) -> Arc<Window> {
    let attr = Window::default_attributes()
        .with_inner_size(LogicalSize::new(768, 768))
        .with_resizable(true)
        .with_title("Protography");
    Arc::new(event_loop.create_window(attr).unwrap())
}

/// Helper function that creates a vello `Renderer` for a given `RenderContext` and `RenderSurface`
fn create_vello_renderer(render_cx: &RenderContext, surface: &RenderSurface<'_>) -> Renderer {
    Renderer::new(
        &render_cx.devices[surface.dev_id].device,
        RendererOptions::default(),
    )
    .expect("Couldn't create renderer")
}

#[derive(Default)]
pub struct Input {
    is_primary_pressed: bool,
    is_secondary_pressed: bool,

    mouse_dx: f64,
    mouse_dy: f64,

    mouse_wheel_dx: f64,
    mouse_wheel_dy: f64,

    is_space_pressed: bool,
    is_shift_pressed: bool,
}
