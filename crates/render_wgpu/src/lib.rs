//! Mobile-safe `wgpu 22.1` forward renderer for Inkbound Rooftops.
//!
//! Renders:
//! 1. Stylized white-plaster walls & bright orange tiled roofs with 2-band cel shading & overcast fog
//! 2. Inverted-hull charcoal ink outlines (configurable via graphics preset)
//! 3. Batched paint strokes, active paint platforms, and Ink Wraith silhouettes (with split VFX)
//! 4. Foreground first-person sword mesh with swing & charge animation
//! 5. 2D sketchbook HUD & menus

use ai::{InkWraith, WraithState};
use bytemuck::{Pod, Zeroable};
use engine_core::{Aabb, GameSettings, Palette};
use gameplay_combat::CombatState;
use gameplay_paint::PaintSystem;
use glam::{Mat4, Vec3};
use std::sync::Arc;
use ui::UiQuad;
use wgpu::util::DeviceExt;
use winit::window::Window;
use world_streaming::{LodLevel, MaterialKind, WorldStreamer};

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct WorldVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub color: [f32; 4],
}

impl WorldVertex {
    const ATTRIBS: [wgpu::VertexAttribute; 3] =
        wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x4];

    fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBS,
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct HudVertex {
    pub position: [f32; 2],
    pub color: [f32; 4],
}

impl HudVertex {
    const ATTRIBS: [wgpu::VertexAttribute; 2] =
        wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x4];

    fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBS,
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct CameraUniform {
    view_proj: [[f32; 4]; 4],
    camera_pos: [f32; 4],
    fx_params: [f32; 4],
}

pub struct SketchRenderer {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    depth_view: wgpu::TextureView,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    world_pipeline: wgpu::RenderPipeline,
    outline_pipeline: wgpu::RenderPipeline,
    hud_pipeline: wgpu::RenderPipeline,
    pub size: (u32, u32),
}

impl SketchRenderer {
    pub async fn new(window: Arc<Window>) -> Result<Self, String> {
        let inner = window.inner_size();
        let width = inner.width.max(1);
        let height = inner.height.max(1);

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..Default::default()
        });

        let surface = instance
            .create_surface(window)
            .map_err(|e| format!("Failed to create wgpu surface: {e}"))?;

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .ok_or_else(|| "No compatible Vulkan/GPU adapter found".to_string())?;

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("Inkbound Device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::downlevel_webgl2_defaults()
                        .using_resolution(adapter.limits()),
                    ..Default::default()
                },
                None,
            )
            .await
            .map_err(|e| format!("Failed to request GPU device: {e}"))?;

        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width,
            height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        let depth_view = Self::create_depth_view(&device, width, height);

        let camera_uniform = CameraUniform {
            view_proj: Mat4::IDENTITY.to_cols_array_2d(),
            camera_pos: [0.0, 0.0, 0.0, 1.0],
            fx_params: [0.0, 0.25, 0.0, 1.0],
        };
        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera Uniform Buffer"),
            contents: bytemuck::cast_slice(&[camera_uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let camera_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Camera Bind Group Layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Camera Bind Group"),
            layout: &camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
        });

        let world_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("World Sketch Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/world.wgsl").into()),
        });
        let hud_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("HUD Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/hud.wgsl").into()),
        });

        let world_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("World Pipeline Layout"),
            bind_group_layouts: &[&camera_bind_group_layout],
            push_constant_ranges: &[],
        });

        let world_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("World Cel Pipeline"),
            layout: Some(&world_layout),
            vertex: wgpu::VertexState {
                module: &world_shader,
                entry_point: "vs_main",
                buffers: &[WorldVertex::layout()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &world_shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: Some(wgpu::Face::Back),
                front_face: wgpu::FrontFace::Ccw,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth24Plus,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let outline_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Charcoal Outline Pipeline"),
            layout: Some(&world_layout),
            vertex: wgpu::VertexState {
                module: &world_shader,
                entry_point: "vs_outline",
                buffers: &[WorldVertex::layout()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &world_shader,
                entry_point: "fs_outline",
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: Some(wgpu::Face::Front),
                front_face: wgpu::FrontFace::Ccw,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth24Plus,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let hud_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("HUD Pipeline Layout"),
            bind_group_layouts: &[],
            push_constant_ranges: &[],
        });

        let hud_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("HUD Pipeline"),
            layout: Some(&hud_layout),
            vertex: wgpu::VertexState {
                module: &hud_shader,
                entry_point: "vs_hud",
                buffers: &[HudVertex::layout()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &hud_shader,
                entry_point: "fs_hud",
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        Ok(Self {
            surface,
            device,
            queue,
            config,
            depth_view,
            camera_buffer,
            camera_bind_group,
            world_pipeline,
            outline_pipeline,
            hud_pipeline,
            size: (width, height),
        })
    }

    fn create_depth_view(device: &wgpu::Device, width: u32, height: u32) -> wgpu::TextureView {
        let tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Depth Texture"),
            size: wgpu::Extent3d {
                width: width.max(1),
                height: height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth24Plus,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        tex.create_view(&wgpu::TextureViewDescriptor::default())
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.size = (width, height);
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
        self.depth_view = Self::create_depth_view(&self.device, width, height);
    }

    pub fn aspect(&self) -> f32 {
        (self.size.0.max(1) as f32) / (self.size.1.max(1) as f32)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn render_frame(
        &mut self,
        view_matrix: Mat4,
        camera_pos: Vec3,
        look_dir: Vec3,
        world: &WorldStreamer,
        enemies: &[InkWraith],
        paint: &PaintSystem,
        combat: &CombatState,
        ui_quads: &[UiQuad],
        settings: &GameSettings,
    ) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let proj = Mat4::perspective_rh(
            settings.fov_degrees.to_radians(),
            self.aspect(),
            0.12,
            260.0,
        );
        let view_proj = proj * view_matrix;

        let uniform = CameraUniform {
            view_proj: view_proj.to_cols_array_2d(),
            camera_pos: [camera_pos.x, camera_pos.y, camera_pos.z, 1.0],
            fx_params: [
                combat.dimensional_rift_intensity,
                combat.rift_plane_angle,
                if settings.reduced_flashes { 1.0 } else { 0.0 },
                if settings.outline_enabled { 1.0 } else { 0.0 },
            ],
        };
        self.queue
            .write_buffer(&self.camera_buffer, 0, bytemuck::cast_slice(&[uniform]));

        // Build world geometry batch (rooftops, towers, enemies, paint strokes/platforms, sword)
        let mut world_verts: Vec<WorldVertex> = Vec::with_capacity(4096);
        for cell in world.cells.values() {
            if cell.lod == LodLevel::Unloaded {
                continue;
            }
            for block in &cell.blocks {
                let color = match block.material {
                    MaterialKind::WhitePlasterWall => Palette::PAPER,
                    MaterialKind::StoneTrim => Palette::STONE,
                    MaterialKind::OrangeTiledRoof => Palette::ROOF_ORANGE,
                    MaterialKind::InkWindowAccent => Palette::INK,
                    MaterialKind::CheckpointShrine => Palette::PAINT_TEAL,
                    MaterialKind::SlashableProp => Palette::INK,
                };
                if block.pitched_roof {
                    push_pitched_roof(&mut world_verts, block.bounds, color);
                } else {
                    push_box(&mut world_verts, block.bounds, color);
                }
            }
        }

        // Render active paint platforms & decals
        for anchor in &paint.platform_anchors {
            if anchor.active_remaining_secs > 0.0 {
                push_box(&mut world_verts, anchor.bounds, Palette::PAINT_TEAL);
            }
        }
        for stroke in &paint.strokes {
            let half = Vec3::splat(stroke.radius * 0.45);
            push_box(
                &mut world_verts,
                Aabb::from_center_half_extents(stroke.position, half),
                stroke.color.rgba(),
            );
        }

        // Render Ink Wraiths (with dimensional-slash split halves when defeated)
        for wraith in enemies {
            let color = if wraith.is_marked() {
                Palette::ROOF_ORANGE
            } else {
                Palette::INK
            };
            if wraith.state == WraithState::Defeated && wraith.split_offset > 0.0 {
                let off = Vec3::new(wraith.split_offset, 0.0, 0.0);
                let left = Aabb::from_center_half_extents(
                    wraith.position + Vec3::new(0.0, 0.55, 0.0) - off,
                    Vec3::new(0.45, 0.5, 0.45),
                );
                let right = Aabb::from_center_half_extents(
                    wraith.position + Vec3::new(0.0, 1.45, 0.0) + off,
                    Vec3::new(0.45, 0.5, 0.45),
                );
                push_box(&mut world_verts, left, color);
                push_box(&mut world_verts, right, color);
            } else if wraith.state != WraithState::Defeated {
                push_box(&mut world_verts, wraith.collider().bounds, color);
            }
        }

        // Render stylized foreground sword blade in camera-relative world space
        push_foreground_sword(&mut world_verts, camera_pos, look_dir, combat);

        let world_vbuf = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("World Vertex Buffer"),
                contents: bytemuck::cast_slice(&world_verts),
                usage: wgpu::BufferUsages::VERTEX,
            });

        // Build 2D HUD quads
        let mut hud_verts: Vec<HudVertex> = Vec::with_capacity(ui_quads.len() * 6);
        for q in ui_quads {
            let x0 = q.x;
            let y0 = q.y;
            let x1 = q.x + q.w;
            let y1 = q.y + q.h;
            let c = q.color;
            hud_verts.extend_from_slice(&[
                HudVertex { position: [x0, y0], color: c },
                HudVertex { position: [x0, y1], color: c },
                HudVertex { position: [x1, y1], color: c },
                HudVertex { position: [x0, y0], color: c },
                HudVertex { position: [x1, y1], color: c },
                HudVertex { position: [x1, y0], color: c },
            ]);
        }

        let hud_vbuf = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("HUD Vertex Buffer"),
                contents: bytemuck::cast_slice(&hud_verts),
                usage: wgpu::BufferUsages::VERTEX,
            });

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Inkbound Frame Encoder"),
            });

        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("3D Sketchbook World Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: Palette::OVERCAST_FOG[0] as f64,
                            g: Palette::OVERCAST_FOG[1] as f64,
                            b: Palette::OVERCAST_FOG[2] as f64,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            if !world_verts.is_empty() {
                rpass.set_bind_group(0, &self.camera_bind_group, &[]);
                rpass.set_vertex_buffer(0, world_vbuf.slice(..));

                // 1. Opaque cel-shaded world pass
                rpass.set_pipeline(&self.world_pipeline);
                rpass.draw(0..world_verts.len() as u32, 0..1);

                // 2. Inverted-hull charcoal ink outline pass
                if settings.outline_enabled {
                    rpass.set_pipeline(&self.outline_pipeline);
                    rpass.draw(0..world_verts.len() as u32, 0..1);
                }
            }
        }

        // 3. 2D Sketchbook HUD & Menu pass
        if !hud_verts.is_empty() {
            let mut hpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("2D HUD Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            hpass.set_pipeline(&self.hud_pipeline);
            hpass.set_vertex_buffer(0, hud_vbuf.slice(..));
            hpass.draw(0..hud_verts.len() as u32, 0..1);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();
        Ok(())
    }
}

fn push_quad(out: &mut Vec<WorldVertex>, p0: Vec3, p1: Vec3, p2: Vec3, p3: Vec3, n: Vec3, color: [f32; 4]) {
    let na = n.to_array();
    let v = |p: Vec3| WorldVertex {
        position: p.to_array(),
        normal: na,
        color,
    };
    out.extend_from_slice(&[v(p0), v(p1), v(p2), v(p0), v(p2), v(p3)]);
}

fn push_box(out: &mut Vec<WorldVertex>, b: Aabb, color: [f32; 4]) {
    let (x0, y0, z0) = (b.min.x, b.min.y, b.min.z);
    let (x1, y1, z1) = (b.max.x, b.max.y, b.max.z);

    // +Y top
    push_quad(out, Vec3::new(x0, y1, z1), Vec3::new(x1, y1, z1), Vec3::new(x1, y1, z0), Vec3::new(x0, y1, z0), Vec3::Y, color);
    // -Y bottom
    push_quad(out, Vec3::new(x0, y0, z0), Vec3::new(x1, y0, z0), Vec3::new(x1, y0, z1), Vec3::new(x0, y0, z1), Vec3::NEG_Y, color);
    // +Z front
    push_quad(out, Vec3::new(x0, y0, z1), Vec3::new(x1, y0, z1), Vec3::new(x1, y1, z1), Vec3::new(x0, y1, z1), Vec3::Z, color);
    // -Z back
    push_quad(out, Vec3::new(x1, y0, z0), Vec3::new(x0, y0, z0), Vec3::new(x0, y1, z0), Vec3::new(x1, y1, z0), Vec3::NEG_Z, color);
    // +X right
    push_quad(out, Vec3::new(x1, y0, z1), Vec3::new(x1, y0, z0), Vec3::new(x1, y1, z0), Vec3::new(x1, y1, z1), Vec3::X, color);
    // -X left
    push_quad(out, Vec3::new(x0, y0, z0), Vec3::new(x0, y0, z1), Vec3::new(x0, y1, z1), Vec3::new(x0, y1, z0), Vec3::NEG_X, color);
}

fn push_pitched_roof(out: &mut Vec<WorldVertex>, b: Aabb, color: [f32; 4]) {
    let (x0, y0, z0) = (b.min.x, b.min.y, b.min.z);
    let (x1, y1, z1) = (b.max.x, b.max.y, b.max.z);
    let apex = Vec3::new((x0 + x1) * 0.5, y1, (z0 + z1) * 0.5);
    let c0 = Vec3::new(x0, y0, z1);
    let c1 = Vec3::new(x1, y0, z1);
    let c2 = Vec3::new(x1, y0, z0);
    let c3 = Vec3::new(x0, y0, z0);

    let mut tri = |a: Vec3, b_pt: Vec3, c: Vec3| {
        let n = (b_pt - a).cross(c - a).normalize_or_zero().to_array();
        out.extend_from_slice(&[
            WorldVertex { position: a.to_array(), normal: n, color },
            WorldVertex { position: b_pt.to_array(), normal: n, color },
            WorldVertex { position: c.to_array(), normal: n, color },
        ]);
    };
    tri(c0, c1, apex);
    tri(c1, c2, apex);
    tri(c2, c3, apex);
    tri(c3, c0, apex);
}

fn push_foreground_sword(
    out: &mut Vec<WorldVertex>,
    camera_pos: Vec3,
    look_dir: Vec3,
    combat: &CombatState,
) {
    let fwd = look_dir.normalize_or_zero();
    let right = fwd.cross(Vec3::Y).normalize_or_zero();
    let up = right.cross(fwd).normalize_or_zero();

    let swing_t = (combat.swing_anim_timer * 4.0).clamp(0.0, 1.0);
    let charge_pull = combat.charge_ratio * 0.14;
    let base = camera_pos
        + fwd * (0.62 - charge_pull)
        + right * (0.28 - swing_t * 0.36)
        + up * (-0.22 + swing_t * 0.14);

    let blade_color = if combat.charge_ratio > 0.65 {
        Palette::ROOF_ORANGE
    } else {
        Palette::PAPER
    };
    let half = Vec3::new(0.03, 0.24, 0.03);
    push_box(out, Aabb::from_center_half_extents(base, half), blade_color);
}
