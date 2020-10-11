use egui::demos::DemoWindow;
use egui::paint::FontDefinitions;
use egui_winit::WinitPlatformDescriptor;
use futures_lite::future::block_on;
use std::iter;
use std::sync::Arc;
use std::time::Instant;
use winit::event::Event::*;
use winit::event_loop::ControlFlow;

const INITIAL_WIDTH: u32 = 1920;
const INITIAL_HEIGHT: u32 = 1080;
const OUTPUT_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8Unorm;

/// A simple egui + wgpu + winit based example.
fn main() {
    let event_loop = winit::event_loop::EventLoop::new();
    let window = winit::window::WindowBuilder::new()
        .with_decorations(true)
        .with_resizable(true)
        .with_transparent(false)
        .with_title("egui-wgpu_winit example")
        .with_inner_size(winit::dpi::PhysicalSize {
            width: INITIAL_WIDTH as f64,
            height: INITIAL_HEIGHT as f64,
        })
        .build(&event_loop)
        .unwrap();

    let instance = wgpu::Instance::new(wgpu::BackendBit::PRIMARY);
    let surface = unsafe { instance.create_surface(&window) };

    let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: Some(&surface),
    }))
    .unwrap();

    let (mut device, mut queue) = block_on(adapter.request_device(
        &wgpu::DeviceDescriptor {
            features: wgpu::Features::PUSH_CONSTANTS,
            limits: wgpu::Limits {
                max_push_constant_size: 8,
                ..Default::default()
            },
            shader_validation: true,
        },
        None,
    ))
    .unwrap();

    let size = window.inner_size();
    let mut sc_desc = wgpu::SwapChainDescriptor {
        usage: wgpu::TextureUsage::OUTPUT_ATTACHMENT,
        format: OUTPUT_FORMAT,
        width: size.width as u32,
        height: size.height as u32,
        present_mode: wgpu::PresentMode::Fifo,
    };
    let mut swap_chain = device.create_swap_chain(&surface, &sc_desc);

    // We use the egui_winit crate as the platform.
    let mut platform = egui_winit::WinitPlatform::new(WinitPlatformDescriptor {
        scale_factor: window.scale_factor(),
        font_definitions: FontDefinitions::with_pixels_per_point(14.0),
        style: Default::default(),
    });

    // We use the egui_wgpu crate as the render backend.
    let mut egui_rpass = egui_wgpu::EguiRenderPass::new(&device, OUTPUT_FORMAT);

    // Use a simple demo UI.
    let mut app = DemoApp::default();

    let start_time = Instant::now();
    event_loop.run(move |event, _, control_flow| {
        platform.handle_event(&event);

        match event {
            RedrawRequested(..) => {
                platform.update_time(start_time.elapsed().as_nanos());

                let output_frame = match swap_chain.get_current_frame() {
                    Ok(frame) => frame,
                    Err(e) => {
                        eprintln!("Dropped frame with error: {}", e);
                        return;
                    }
                };

                let mut ui = platform.begin_frame();

                // Draw the egui based UI.
                app.ui(&mut ui);

                // The outputs could be inspected now and handled.
                let (_output, paint_jobs) = platform.end_frame();

                let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("encoder"),
                });

                // Upload all resources for the GPU.
                egui_rpass.update_texture(&device, &queue, &platform.context().texture());
                egui_rpass.update_buffers(&mut device, &mut queue, &paint_jobs);

                // Record all render passes.
                egui_rpass.execute(
                    &mut encoder,
                    &output_frame.output.view,
                    &paint_jobs,
                    sc_desc.width,
                    sc_desc.height,
                    window.scale_factor() as f32,
                    true,
                );

                // Submit the commands.
                queue.submit(iter::once(encoder.finish()));
                *control_flow = ControlFlow::Poll;
            }
            MainEventsCleared => {
                window.request_redraw();
                *control_flow = ControlFlow::Wait;
            }
            WindowEvent { event, .. } => match event {
                winit::event::WindowEvent::Resized(size) => {
                    sc_desc.width = size.width;
                    sc_desc.height = size.height;
                    swap_chain = device.create_swap_chain(&surface, &sc_desc);
                }
                winit::event::WindowEvent::CloseRequested => {
                    *control_flow = ControlFlow::Exit;
                }
                _ => {}
            },
            _ => (),
        }
    });
}

/// A simple demonstration how to draw with egui.
pub struct DemoApp {
    demo_window: egui::demos::DemoWindow,
}

impl DemoApp {
    pub fn ui(&mut self, mut ui: &mut egui::Ui) {
        self.show_menu_bar(&mut ui);
        self.windows(ui.ctx());
    }

    /// Show the open windows.
    fn windows(&mut self, ctx: &Arc<egui::Context>) {
        egui::Window::new("Demo").scroll(true).show(ctx, |ui| {
            self.demo_window.ui(ui);
        });

        egui::Window::new("Settings").show(ctx, |ui| {
            ctx.settings_ui(ui);
        });

        egui::Window::new("Inspection")
            .scroll(true)
            .show(ctx, |ui| {
                ctx.inspection_ui(ui);
            });

        egui::Window::new("Memory")
            .resizable(false)
            .show(ctx, |ui| {
                ctx.memory_ui(ui);
            });

        self.resize_windows(ctx);
    }

    fn show_menu_bar(&self, ui: &mut egui::Ui) {
        egui::menu::bar(ui, |ui| {
            egui::menu::menu(ui, "File", |ui| {
                if ui.button("Reorganize windows").clicked {
                    ui.ctx().memory().reset_areas();
                }
                if ui
                    .button("Clear entire Egui memory")
                    .on_hover_text("Forget scroll, collapsibles etc")
                    .clicked
                {
                    *ui.ctx().memory() = Default::default();
                }
            });
            egui::menu::menu(ui, "About", |ui| {
                ui.label("This is Egui");
                ui.add(
                    egui::Hyperlink::new("https://github.com/emilk/egui").text("Egui home page"),
                );
            });
        });
    }

    fn resize_windows(&mut self, ctx: &Arc<egui::Context>) {
        egui::Window::new("resizable")
            .scroll(false)
            .resizable(true)
            .show(ctx, |ui| {
                ui.label("scroll:    NO");
                ui.label("resizable: YES");
                ui.label(egui::demos::LOREM_IPSUM);
            });

        egui::Window::new("resizable + embedded scroll")
            .scroll(false)
            .resizable(true)
            .default_height(300.0)
            .show(ctx, |ui| {
                ui.label("scroll:    NO");
                ui.label("resizable: YES");
                ui.heading("We have a sub-region with scroll bar:");
                egui::ScrollArea::auto_sized().show(ui, |ui| {
                    ui.label(egui::demos::LOREM_IPSUM_LONG);
                    ui.label(egui::demos::LOREM_IPSUM_LONG);
                });
                // ui.heading("Some additional text here, that should also be visible"); // this works, but messes with the resizing a bit
            });

        egui::Window::new("resizable + scroll")
            .scroll(true)
            .resizable(true)
            .default_height(300.0)
            .show(ctx, |ui| {
                ui.label("scroll:    YES");
                ui.label("resizable: YES");
                ui.label(egui::demos::LOREM_IPSUM_LONG);
            });

        egui::Window::new("auto_sized")
            .auto_sized()
            .show(ctx, |ui| {
                ui.label("This window will auto-size based on its contents.");
                ui.heading("Resize this area:");
                egui::Resize::default().show(ui, |ui| {
                    ui.label(egui::demos::LOREM_IPSUM);
                });
                ui.heading("Resize the above area!");
            });
    }
}

impl Default for DemoApp {
    fn default() -> Self {
        Self {
            demo_window: DemoWindow::default(),
        }
    }
}
