#[path = "../application_framework.rs"]
mod application_framework;

use contrast_renderer::renderer::RenderOperation;
use geometric_algebra::{
    ppga3d::{Rotor, Translator},
    One,
};

const OPEN_SANS_TTF: &[u8] = include_bytes!("../fonts/OpenSans-Regular.ttf");
const MSAA_SAMPLE_COUNT: u32 = 4;

struct Application {
    depth_stencil_texture_view: Option<wgpu::TextureView>,
    msaa_color_texture_view: Option<wgpu::TextureView>,
    renderer: contrast_renderer::renderer::Renderer,
    dynamic_stroke_options: [contrast_renderer::path::DynamicStrokeOptions; 1],
    instance_buffers: [contrast_renderer::renderer::Buffer; 2],
    shape: contrast_renderer::renderer::Shape,
    viewport_size: wgpu::Extent3d,
    view_rotation: Rotor,
    view_distance: f32,
}

impl application_framework::Application for Application {
    fn new(device: &wgpu::Device, _queue: &mut wgpu::Queue, surface_configuration: &wgpu::SurfaceConfiguration) -> Self {
        let blending = wgpu::ColorTargetState {
            format: surface_configuration.format,
            blend: Some(wgpu::BlendState {
                color: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::SrcAlpha,
                    dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                    operation: wgpu::BlendOperation::Add,
                },
                alpha: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::SrcAlpha,
                    dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                    operation: wgpu::BlendOperation::Add,
                },
            }),
            write_mask: wgpu::ColorWrites::ALL,
        };
        let renderer = contrast_renderer::renderer::Renderer::new(&device, blending, MSAA_SAMPLE_COUNT, 4, 4).unwrap();

        let dynamic_stroke_options = [contrast_renderer::path::DynamicStrokeOptions::Dashed {
            join: contrast_renderer::path::Join::Miter,
            pattern: vec![contrast_renderer::path::DashInterval {
                gap_start: 3.0.into(),
                gap_end: 4.0.into(),
                dash_start: contrast_renderer::path::Cap::Butt,
                dash_end: contrast_renderer::path::Cap::Butt,
            }],
            phase: 0.0.into(),
        }];

        let font_face = ttf_parser::Face::from_slice(OPEN_SANS_TTF, 0).unwrap();
        let mut paths = contrast_renderer::text::paths_of_text(
            &font_face,
            &contrast_renderer::text::Layout {
                size: 2.7.into(),
                orientation: contrast_renderer::text::Orientation::LeftToRight,
                major_alignment: contrast_renderer::text::Alignment::Center,
                minor_alignment: contrast_renderer::text::Alignment::Center,
            },
            "Hello World",
            None,
        );
        for path in &mut paths {
            path.reverse();
        }
        paths.insert(0, contrast_renderer::path::Path::from_rounded_rect([0.0, 0.0], [5.8, 1.3], 0.5));
        paths[0].stroke_options = Some(contrast_renderer::path::StrokeOptions {
            width: 0.1.into(),
            offset: 0.0.into(),
            miter_clip: 1.0.into(),
            closed: true,
            dynamic_stroke_options_group: 0,
            curve_approximation: contrast_renderer::path::CurveApproximation::UniformTangentAngle(0.1.into()),
        });
        let shape = contrast_renderer::renderer::Shape::from_paths(&device, &renderer, &dynamic_stroke_options, paths.as_slice(), None).unwrap();

        Self {
            depth_stencil_texture_view: None,
            msaa_color_texture_view: None,
            renderer,
            dynamic_stroke_options,
            instance_buffers: [
                contrast_renderer::renderer::Buffer::new(device, wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST, &[]),
                contrast_renderer::renderer::Buffer::new(device, wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST, &[]),
            ],
            shape,
            viewport_size: wgpu::Extent3d::default(),
            view_rotation: Rotor::one(),
            view_distance: 5.0,
        }
    }

    fn resize(&mut self, device: &wgpu::Device, _queue: &mut wgpu::Queue, surface_configuration: &wgpu::SurfaceConfiguration) {
        self.viewport_size = wgpu::Extent3d {
            width: surface_configuration.width,
            height: surface_configuration.height,
            depth_or_array_layers: 1,
        };
        let depth_stencil_texture_descriptor = wgpu::TextureDescriptor {
            size: self.viewport_size,
            mip_level_count: 1,
            sample_count: MSAA_SAMPLE_COUNT,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth24PlusStencil8,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            label: None,
        };
        let depth_stencil_texture = device.create_texture(&depth_stencil_texture_descriptor);
        self.depth_stencil_texture_view = Some(depth_stencil_texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2),
            ..wgpu::TextureViewDescriptor::default()
        }));
        if MSAA_SAMPLE_COUNT > 1 {
            let msaa_color_texture_descriptor = wgpu::TextureDescriptor {
                size: self.viewport_size,
                mip_level_count: 1,
                sample_count: MSAA_SAMPLE_COUNT,
                dimension: wgpu::TextureDimension::D2,
                format: surface_configuration.format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                label: None,
            };
            let msaa_color_texture = device.create_texture(&msaa_color_texture_descriptor);
            self.msaa_color_texture_view = Some(msaa_color_texture.create_view(&wgpu::TextureViewDescriptor {
                dimension: Some(wgpu::TextureViewDimension::D2),
                ..wgpu::TextureViewDescriptor::default()
            }));
        }
    }

    fn render(&mut self, device: &wgpu::Device, queue: &mut wgpu::Queue, frame: &wgpu::SurfaceTexture, animation_time: f64) {
        match &mut self.dynamic_stroke_options[0] {
            contrast_renderer::path::DynamicStrokeOptions::Dashed { phase, .. } => {
                *phase = (animation_time as f32 * 2.0).into();
            }
            _ => unreachable!(),
        }
        self.shape.set_dynamic_stroke_options(queue, 0, &self.dynamic_stroke_options[0]).unwrap();
        let projection_matrix = contrast_renderer::utils::matrix_multiplication(
            &contrast_renderer::utils::perspective_projection(
                std::f32::consts::PI * 0.5,
                self.viewport_size.width as f32 / self.viewport_size.height as f32,
                1.0,
                1000.0,
            ),
            &contrast_renderer::utils::motor3d_to_mat4(
                &(Translator {
                    g0: [1.0, 0.0, 0.0, -0.5 * self.view_distance].into(),
                } * self.view_rotation),
            ),
        );
        let instances_transform: &[[geometric_algebra::ppga3d::Point; 4]] = &[projection_matrix];
        let instances_color: &[contrast_renderer::renderer::Color] = &[[1.0, 1.0, 1.0, 1.0].into()];
        self.instance_buffers[0].update(device, queue, &contrast_renderer::concat_buffers!([instances_transform]).1);
        self.instance_buffers[1].update(device, queue, &contrast_renderer::concat_buffers!([instances_color]).1);
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_stencil_texture_view.as_ref().unwrap(),
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(0.0),
                        store: false,
                    }),
                    stencil_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(0),
                        store: true,
                    }),
                }),
            });
            render_pass.set_vertex_buffer(0, self.instance_buffers[0].buffer.slice(..));
            self.shape.render(&self.renderer, &mut render_pass, 0..1, RenderOperation::Stencil);
        }
        {
            let frame_view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[wgpu::RenderPassColorAttachment {
                    view: if MSAA_SAMPLE_COUNT == 1 {
                        &frame_view
                    } else {
                        &self.msaa_color_texture_view.as_ref().unwrap()
                    },
                    resolve_target: if MSAA_SAMPLE_COUNT == 1 { None } else { Some(&frame_view) },
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: true,
                    },
                }],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_stencil_texture_view.as_ref().unwrap(),
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: false,
                    }),
                    stencil_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: true,
                    }),
                }),
            });
            render_pass.set_vertex_buffer(0, self.instance_buffers[0].buffer.slice(..));
            render_pass.set_vertex_buffer(1, self.instance_buffers[1].buffer.slice(..));
            self.shape.render(&self.renderer, &mut render_pass, 0..1, RenderOperation::Color);
        }
        queue.submit(Some(encoder.finish()));
    }

    fn window_event(&mut self, event: winit::event::WindowEvent) {
        match event {
            winit::event::WindowEvent::CursorMoved { position, .. } => {
                let position = [
                    std::f32::consts::PI * (position.x as f32 / self.viewport_size.width as f32 - 0.5),
                    std::f32::consts::PI * (position.y as f32 / self.viewport_size.height as f32 - 0.5),
                ];
                self.view_rotation = contrast_renderer::utils::rotate_around_axis(position[0], &[0.0, 1.0, 0.0]);
                self.view_rotation *= contrast_renderer::utils::rotate_around_axis(position[1], &[1.0, 0.0, 0.0]);
            }
            winit::event::WindowEvent::MouseWheel { delta, .. } => {
                let difference = match delta {
                    winit::event::MouseScrollDelta::LineDelta(x, y) => [x, y],
                    winit::event::MouseScrollDelta::PixelDelta(delta) => [delta.x as f32 * 0.1, delta.y as f32 * 0.1],
                };
                self.view_distance = (self.view_distance + difference[1]).clamp(2.0, 100.0);
            }
            _ => {}
        }
    }
}

fn main() {
    application_framework::ApplicationManager::run::<Application>("Contrast Renderer - Showcase");
}
