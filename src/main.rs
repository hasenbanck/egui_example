use std::iter;
use std::time::Instant;

use egui::paint::FontDefinitions;
use egui_wgpu_backend::EguiRenderPass;
use egui_winit_platform::{WinitPlatform, WinitPlatformDescriptor};
use futures_lite::future::block_on;
use winit::event::Event::*;
use winit::event_loop::ControlFlow;

const INITIAL_WIDTH: u32 = 1920;
const INITIAL_HEIGHT: u32 = 1080;
const OUTPUT_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8UnormSrgb;

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
        present_mode: wgpu::PresentMode::Mailbox,
    };
    let mut swap_chain = device.create_swap_chain(&surface, &sc_desc);

    // We use the egui_winit crate as the platform.
    let mut platform = WinitPlatform::new(WinitPlatformDescriptor {
        scale_factor: window.scale_factor(),
        font_definitions: FontDefinitions::with_pixels_per_point(window.scale_factor() as f32),
        style: Default::default(),
    });

    // We use the egui_wgpu crate as the render backend.
    let mut egui_rpass = EguiRenderPass::new(&device, OUTPUT_FORMAT);

    // Display simple demo window.
    let mut demo_window = egui::demos::DemoWindow::default();

    let start_time = Instant::now();
    event_loop.run(move |event, _, control_flow| {
        platform.handle_event(&event);

        match event {
            RedrawRequested(..) => {
                platform.update_time(start_time.elapsed().as_secs_f64());

                let output_frame = match swap_chain.get_current_frame() {
                    Ok(frame) => frame,
                    Err(e) => {
                        eprintln!("Dropped frame with error: {}", e);
                        return;
                    }
                };

                // Begin to draw the UI frame.
                let ui = platform.begin_frame();

                // Draw the egui based UI.
                egui::Window::new("Demo").scroll(true).show(ui.ctx(), |ui| {
                    demo_window.ui(ui);
                });

                // End the UI frame. We could now handle the output and draw the UI with the backend.
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
                    Some(wgpu::Color::BLACK),
                );

                // Submit the commands.
                queue.submit(iter::once(encoder.finish()));
                *control_flow = ControlFlow::Poll;
            }
            MainEventsCleared => {
                window.request_redraw();
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
