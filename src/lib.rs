use std::num::NonZeroUsize;
use std::sync::Arc;
use web_time::Instant;

use wasm_bindgen::prelude::*;

use log::info;

use vello::kurbo::{Affine, Rect, Stroke};
use vello::peniko::{color::palette, Color};
use vello::util::{RenderContext, RenderSurface};
use vello::{AaConfig, Renderer, RendererOptions, Scene};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowAttributes, WindowId};
use winit::dpi::PhysicalSize;

use vello::wgpu;

const MARGIN: f64 = 50.0;

struct VelloClient<'a> {
    surface: RenderSurface<'a>,
    window: Arc<Window>,
    context: RenderContext,
    renderers: Vec<Option<Renderer>>,
    scene: Scene,
}

impl ApplicationHandler for VelloClient<'_> {
    fn resumed(&mut self, _event_loop: &winit::event_loop::ActiveEventLoop) {}

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                info!("Closing");
                event_loop.exit();
            }

            WindowEvent::Resized(size) => {
                self.context
                    .resize_surface(&mut self.surface, size.width, size.height);
                self.window.request_redraw();
                info!("Resize to {}, {}", size.width, size.height);
            }

            WindowEvent::RedrawRequested => {
                // Get the window size
                let width = self.surface.config.width - 20;
                let height = self.surface.config.height - 20;

                // Draw the output into the scene.
                let start = Instant::now();
				self.scene.reset();
				let rect = Rect::new(MARGIN, MARGIN, width as f64 - MARGIN * 2.0, height as f64 - MARGIN * 2.0);
        		self.scene.stroke(
            		&Stroke::new(1.0),
            		Affine::IDENTITY,
            		Color::BLACK,
            		None,
            		&rect,
        		);

                // Get a handle to the device
                let device_handle = &self.context.devices[self.surface.dev_id];

                // Get the surface's texture
                let surface_texture = self
                    .surface
                    .surface
                    .get_current_texture()
                    .expect("failed to get surface texture");

                // Render to the surface's texture
                self.renderers[self.surface.dev_id]
                    .as_mut()
                    .unwrap()
                    .render_to_surface(
                        &device_handle.device,
                        &device_handle.queue,
                        &self.scene,
                        &surface_texture,
                        &vello::RenderParams {
                            base_color: palette::css::WHITE, // Background color
                            width,
                            height,
                            antialiasing_method: AaConfig::Msaa16,
                        },
                    )
                    .expect("failed to render to surface");
                info!("Render complete, time = {:2?}", Instant::now() - start);

                // Queue the texture to be presented on the surface
                surface_texture.present();
                info!("surface present, time = {:2?}", Instant::now() - start);

                device_handle.device.poll(wgpu::Maintain::Poll);
                info!("After device poll, time = {:2?}", Instant::now() - start);
            }
            _ => (),
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        // Check for updates, redraw if necessary.
    }
}

fn display_error_message() -> Option<()> {
    let window = web_sys::window()?;
    let document = window.document()?;
    let elements = document.get_elements_by_tag_name("body");
    let body = elements.item(0)?;
    body.set_inner_html(
        r#"<style>
        p {
            margin: 2em 10em;
            font-family: sans-serif;
        }
        </style>
        <p><a href="https://caniuse.com/webgpu">WebGPU</a>
        is not enabled. Make sure your browser is updated to
        <a href="https://chromiumdash.appspot.com/schedule">Chrome M113</a> or
        another browser compatible with WebGPU.</p>"#,
    );
    Some(())
}

fn run(
    event_loop: EventLoop<()>,
    render_cx: RenderContext,
    surface: RenderSurface<'_>,
    window: Arc<Window>,
) {
    let renderers = {
        let mut renderers = vec![];
        renderers.resize_with(render_cx.devices.len(), || None);
        let id = surface.dev_id;
        let renderer = Renderer::new(
            &render_cx.devices[id].device,
            RendererOptions {
                surface_format: Some(surface.format),
                use_cpu: false,
                antialiasing_support: vello::AaSupport::all(),
                // We currently initialise on one thread on WASM, but mark this here
                // anyway
                num_init_threads: NonZeroUsize::new(1),
            },
        )
        .map_err(|e| {
            // Pretty-print any renderer creation error using Display formatting before unwrapping.
            eprintln!("{e}");
            e
        })
        .expect("Failed to create renderer");
        renderers[id] = Some(renderer);
        renderers
    };

    let mut app = VelloClient {
        surface: surface,
        window: window,
        context: render_cx,
        renderers: renderers,
        scene: Scene::new(),
    };

    event_loop.run_app(&mut app).expect("run to completion");
}

fn window_attributes() -> WindowAttributes {
    Window::default_attributes()
        //.with_inner_size(LogicalSize::new(1044, 800))
        .with_resizable(true)
        .with_title("Vello test client")
}

#[wasm_bindgen(start)]
pub fn start_app() {
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));
    console_log::init().expect("could not initialize logger");
	if let Err(e) = run_app() {
    	info!("run_app error: {}", e);
	} else {
    	info!("run_app exit with no error");
	}
}

pub fn run_app() -> Result<(), Box<dyn std::error::Error>> {
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Wait);
    let render_cx = RenderContext::new();
    let mut render_cx = render_cx;
    use winit::platform::web::WindowExtWebSys;
    #[allow(deprecated)]
    let window = Arc::new(event_loop.create_window(window_attributes()).unwrap());
    // On wasm, append the canvas to the document body
    let canvas = window.canvas().unwrap();
    web_sys::window()
        .and_then(|win| win.document())
        .and_then(|doc| doc.body())
        .and_then(|body| body.append_child(canvas.as_ref()).ok())
        .expect("couldn't append canvas to document body");
    // Best effort to start with the canvas focused, taking input
    drop(web_sys::HtmlElement::from(canvas).focus());
    wasm_bindgen_futures::spawn_local(async move {
        let (width, height, scale_factor) = web_sys::window()
            .map(|w| {
                (
                    w.inner_width().unwrap().as_f64().unwrap(),
                    w.inner_height().unwrap().as_f64().unwrap(),
                    w.device_pixel_ratio(),
                )
            })
            .unwrap();
        info!("Window {} x {}, scale {}", width, height, scale_factor);
        let size: PhysicalSize<u32> = PhysicalSize::from_logical::<_, f64>((width, height), scale_factor);
        if let Some(sz) =  window.request_inner_size(size) {
			info!("Request inner size: {} x {}", sz.width, sz.height);
		} else {
			info!("Resize deferred");
		}
        info!("scaled size {} x {}", size.width, size.height);
        let surface = render_cx
            .create_surface(
                window.clone(),
                size.width,
                size.height,
                wgpu::PresentMode::AutoVsync,
            )
            .await;
        if let Ok(surface) = surface {
            // No error handling here; if the event loop has finished, we don't need to send them the surface
            run(event_loop, render_cx, surface, window);
        } else {
            _ = display_error_message();
        }
    });
	Ok(())
}
