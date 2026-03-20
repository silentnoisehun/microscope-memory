use std::sync::Arc;
use std::time::Instant;

use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowAttributes, WindowId};

use microscope_memory::MicroscopeReader;
use microscope_memory::viz::camera::OrbitCamera;
use microscope_memory::viz::edges;
use microscope_memory::viz::picking;
use microscope_memory::viz::renderer::Renderer;
use microscope_memory::viz::scene::SceneData;
use microscope_memory::viz::ui::{self, UiState};

struct App {
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    egui_ctx: egui::Context,
    egui_state: Option<egui_winit::State>,
    camera: OrbitCamera,
    reader: MicroscopeReader,
    scene: Option<SceneData>,
    ui_state: UiState,
    mouse_pressed: bool,
    shift_pressed: bool,
    last_mouse: (f64, f64),
    _last_frame: Instant,
    frame_count: u32,
    fps_timer: Instant,
}

impl App {
    fn new(reader: MicroscopeReader) -> Self {
        Self {
            window: None,
            renderer: None,
            egui_ctx: egui::Context::default(),
            egui_state: None,
            camera: OrbitCamera::new(16.0 / 9.0),
            reader,
            scene: None,
            ui_state: UiState::new(),
            mouse_pressed: false,
            shift_pressed: false,
            last_mouse: (0.0, 0.0),
            _last_frame: Instant::now(),
            frame_count: 0,
            fps_timer: Instant::now(),
        }
    }

    fn rebuild_scene(&mut self) {
        let scene = SceneData::from_reader(&self.reader, &self.ui_state.depth_visible, &self.ui_state.layer_visible);
        self.ui_state.point_count = scene.instances.len();

        if let Some(ref mut renderer) = self.renderer {
            renderer.update_instances(&scene.instances);

            if self.ui_state.show_edges {
                let edge_verts = edges::build_edges(&self.reader, &scene.instances, &scene.block_indices);
                renderer.update_edges(&edge_verts);
            } else {
                renderer.update_edges(&[]);
            }
        }

        self.scene = Some(scene);
        self.ui_state.needs_rebuild = false;
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() { return; }

        let attrs = WindowAttributes::default()
            .with_title("Microscope Memory 3D")
            .with_inner_size(winit::dpi::LogicalSize::new(1280.0, 720.0));

        let window = Arc::new(event_loop.create_window(attrs).expect("create window"));
        let size = window.inner_size();

        let renderer = Renderer::new(window.clone());

        let egui_state = egui_winit::State::new(
            self.egui_ctx.clone(),
            egui::ViewportId::ROOT,
            &window,
            Some(window.scale_factor() as f32),
            None,
            None,
        );

        self.camera.aspect = size.width as f32 / size.height.max(1) as f32;
        self.renderer = Some(renderer);
        self.egui_state = Some(egui_state);
        self.window = Some(window);

        self.rebuild_scene();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        // Forward to egui first
        if let Some(ref mut egui_state) = self.egui_state {
            if let Some(ref window) = self.window {
                let response = egui_state.on_window_event(window, &event);
                if response.consumed { return; }
            }
        }

        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }

            WindowEvent::Resized(size) => {
                if let Some(ref mut renderer) = self.renderer {
                    renderer.resize(size.width, size.height);
                    self.camera.aspect = size.width as f32 / size.height.max(1) as f32;
                }
            }

            WindowEvent::MouseInput { state, button, .. } => {
                match button {
                    MouseButton::Left => {
                        self.mouse_pressed = state == ElementState::Pressed;

                        // Click-to-select on release
                        if state == ElementState::Released {
                            if let (Some(ref scene), Some(ref renderer)) = (&self.scene, &self.renderer) {
                                let vp = glam::Mat4::from_cols_array_2d(&self.camera.uniform().view_proj);
                                if let Some(hit) = picking::pick_point(
                                    self.last_mouse.0 as f32,
                                    self.last_mouse.1 as f32,
                                    renderer.surface_config.width as f32,
                                    renderer.surface_config.height as f32,
                                    &vp,
                                    &scene.instances,
                                ) {
                                    let block_idx = scene.block_indices[hit];
                                    self.ui_state.selected_block = Some(block_idx);
                                    self.ui_state.selected_text = self.reader.text(block_idx).to_string();
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }

            WindowEvent::CursorMoved { position, .. } => {
                let dx = position.x - self.last_mouse.0;
                let dy = position.y - self.last_mouse.1;
                self.last_mouse = (position.x, position.y);

                if self.mouse_pressed {
                    if self.shift_pressed {
                        self.camera.pan(-dx as f32, dy as f32);
                    } else {
                        self.camera.rotate(-dx as f32 * 0.005, -dy as f32 * 0.005);
                    }
                    if let Some(ref window) = self.window {
                        window.request_redraw();
                    }
                }
            }

            WindowEvent::MouseWheel { delta, .. } => {
                let scroll = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(pos) => pos.y as f32 / 50.0,
                };
                self.camera.zoom(scroll);
                if let Some(ref window) = self.window {
                    window.request_redraw();
                }
            }

            WindowEvent::ModifiersChanged(mods) => {
                self.shift_pressed = mods.state().shift_key();
            }

            WindowEvent::RedrawRequested => {
                // FPS tracking
                self.frame_count += 1;
                let elapsed = self.fps_timer.elapsed().as_secs_f32();
                if elapsed >= 1.0 {
                    self.ui_state.fps = self.frame_count as f32 / elapsed;
                    self.frame_count = 0;
                    self.fps_timer = Instant::now();
                }

                if self.ui_state.needs_rebuild {
                    self.rebuild_scene();
                }

                let camera_uniform = self.camera.uniform();

                // egui frame
                let egui_input = if let Some(ref mut egui_state) = self.egui_state {
                    if let Some(ref window) = self.window {
                        egui_state.take_egui_input(window)
                    } else {
                        return;
                    }
                } else {
                    return;
                };

                let full_output = self.egui_ctx.run(egui_input, |ctx| {
                    ui::draw_panels(ctx, &mut self.ui_state, Some(&self.reader));
                });

                if let Some(ref mut egui_state) = self.egui_state {
                    if let Some(ref window) = self.window {
                        egui_state.handle_platform_output(window, full_output.platform_output);
                    }
                }

                let pixels_per_point = self.egui_ctx.pixels_per_point();
                let primitives = self.egui_ctx.tessellate(full_output.shapes, pixels_per_point);

                let screen_descriptor = egui_wgpu::ScreenDescriptor {
                    size_in_pixels: [
                        self.renderer.as_ref().unwrap().surface_config.width,
                        self.renderer.as_ref().unwrap().surface_config.height,
                    ],
                    pixels_per_point,
                };

                if let Some(ref mut renderer) = self.renderer {
                    renderer.render(
                        &camera_uniform,
                        self.ui_state.show_edges,
                        &primitives,
                        &full_output.textures_delta,
                        &screen_descriptor,
                    );
                }
            }

            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(ref window) = self.window {
            window.request_redraw();
        }
    }
}

fn main() {
    println!("Loading microscope memory...");
    let reader = MicroscopeReader::open();
    println!("  {} blocks loaded", reader.block_count);

    let event_loop = EventLoop::new().expect("create event loop");
    let mut app = App::new(reader);
    event_loop.run_app(&mut app).expect("run app");
}
