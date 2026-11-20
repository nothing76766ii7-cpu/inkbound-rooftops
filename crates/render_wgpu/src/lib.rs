//! Mobile-safe `wgpu 22.1` forward renderer for Inkbound Rooftops (PRD v2.0).
//!
//! Renders:
//! 1. Dense medieval sketchbook town (plaster variation, tiled orange roofs, recessed ink windows, chimneys)
//! 2. Depth-scaled inverted-hull charcoal outlines
//! 3. Intact & split sliceable crates, Ink Wraiths (with dimensional split halves), and paint strokes/platforms
//! 4. Articulated animated first-person sword rig (blade + fuller + guard + hilt) driven by locomotion & attack state
//! 5. 3D white-and-ink slash crescent trails, traveling dimensional rift waves, and orange/charcoal impact sparks
//! 6. Crisp 2D vector HUD (circular parchment buttons, pictograms, live joystick, numeric counters, skyline menus)

use ai::{InkWraith, WraithState};
use bytemuck::{Pod, Zeroable};
use engine_core::{Aabb, GameSettings, Palette};
use gameplay_combat::{AttackKind, AttackState, CombatState};
use gameplay_paint::PaintSystem;
use gameplay_player::LocomotionState;
use glam::{Mat4, Vec3};
use std::sync::Arc;
use ui::icons::UiVert;
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
    vfx_pipeline: wgpu::RenderPipeline,
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
            fx_params: [0.0, 0.0, 0.0, 1.0],
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

        // Double-sided translucent VFX pipeline for 3D slash trails, dimensional rift waves, & sparks
        let vfx_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Slash VFX Pipeline"),
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
                cull_mode: None,
                front_face: wgpu::FrontFace::Ccw,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth24Plus,
                depth_write_enabled: false,
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
            vfx_pipeline,
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
        fov_degrees: f32,
        speed_factor: f32,
        bob_phase: f32,
        locomotion: LocomotionState,
        world: &WorldStreamer,
        enemies: &[InkWraith],
        paint: &PaintSystem,
        combat: &CombatState,
        ui_verts: &[UiVert],
        settings: &GameSettings,
    ) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let proj = Mat4::perspective_rh(fov_degrees.to_radians(), self.aspect(), 0.10, 280.0);
        let view_proj = proj * view_matrix;

        let uniform = CameraUniform {
            view_proj: view_proj.to_cols_array_2d(),
            camera_pos: [camera_pos.x, camera_pos.y, camera_pos.z, 1.0],
            fx_params: [
                combat.dimensional_rift_intensity,
                if settings.reduced_motion { 0.0 } else { speed_factor },
                if settings.reduced_flashes { 1.0 } else { 0.0 },
                if settings.outline_enabled { 1.0 } else { 0.0 },
            ],
        };
        self.queue
            .write_buffer(&self.camera_buffer, 0, bytemuck::cast_slice(&[uniform]));

        let mut world_verts: Vec<WorldVertex> = Vec::with_capacity(8192);
        for cell in world.cells.values() {
            if cell.lod == LodLevel::Unloaded {
                continue;
            }
            for block in &cell.blocks {
                let color = match block.material {
                    MaterialKind::WhitePlasterWall => Palette::PAPER,
                    MaterialKind::WarmPlasterWall => [0.935, 0.918, 0.885, 1.0],
                    MaterialKind::StoneTrim => Palette::STONE,
                    MaterialKind::OrangeTiledRoof => Palette::ROOF_ORANGE,
                    MaterialKind::SlateRoofAccent => [0.62, 0.56, 0.54, 1.0],
                    MaterialKind::InkWindowAccent => [0.19, 0.20, 0.23, 1.0],
                    MaterialKind::ChimneyStone => [0.76, 0.74, 0.72, 1.0],
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

        // Render intact & split sliceable crates
        for c in &world.crates {
            let crate_col = [0.78, 0.54, 0.34, 1.0];
            if c.intact {
                push_box(&mut world_verts, c.collider().bounds, crate_col);
            } else if c.cleanup_timer > 0.0 {
                let half = Vec3::new(0.32, 0.55, 0.55);
                push_box(&mut world_verts, Aabb::from_center_half_extents(c.half_a_pos, half), crate_col);
                push_box(&mut world_verts, Aabb::from_center_half_extents(c.half_b_pos, half), crate_col);
            }
        }

        // Render active paint platforms & strokes
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
            } else if wraith.state == WraithState::Stagger {
                Palette::HEALTH_RED
            } else {
                Palette::INK
            };
            if wraith.state == WraithState::Defeated && wraith.split_offset > 0.0 {
                let off = Vec3::new(wraith.split_offset, 0.0, 0.0);
                let left = Aabb::from_center_half_extents(
                    wraith.position + Vec3::new(0.0, 0.55, 0.0) - off,
                    Vec3::new(0.45, 0.50, 0.45),
                );
                let right = Aabb::from_center_half_extents(
                    wraith.position + Vec3::new(0.0, 1.45, 0.0) + off,
                    Vec3::new(0.45, 0.50, 0.45),
                );
                push_box(&mut world_verts, left, color);
                push_box(&mut world_verts, right, color);
            } else if wraith.state != WraithState::Defeated {
                push_box(&mut world_verts, wraith.collider().bounds, color);
            }
        }

        // Render articulated, animated first-person sword rig
        push_articulated_sword_rig(
            &mut world_verts,
            camera_pos,
            look_dir,
            bob_phase,
            locomotion,
            combat,
            settings.reduced_motion,
        );

        // Build translucent 3D VFX batch (slash crescent trails, traveling dimensional rifts, impact sparks)
        let mut vfx_verts: Vec<WorldVertex> = Vec::with_capacity(512);
        push_combat_vfx(&mut vfx_verts, combat);

        let world_vbuf = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("World Vertex Buffer"),
                contents: bytemuck::cast_slice(&world_verts),
                usage: wgpu::BufferUsages::VERTEX,
            });

        let vfx_vbuf = if !vfx_verts.is_empty() {
            Some(self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("VFX Vertex Buffer"),
                contents: bytemuck::cast_slice(&vfx_verts),
                usage: wgpu::BufferUsages::VERTEX,
            }))
        } else {
            None
        };

        let hud_verts: Vec<HudVertex> = ui_verts
            .iter()
            .map(|v| HudVertex {
                position: v.pos,
                color: v.color,
            })
            .collect();
        let hud_vbuf = if !hud_verts.is_empty() {
            Some(self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("HUD Vertex Buffer"),
                contents: bytemuck::cast_slice(&hud_verts),
                usage: wgpu::BufferUsages::VERTEX,
            }))
        } else {
            None
        };

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

                // 1. Opaque cel-shaded sketchbook world pass
                rpass.set_pipeline(&self.world_pipeline);
                rpass.draw(0..world_verts.len() as u32, 0..1);

                // 2. Inverted-hull charcoal ink outline pass
                if settings.outline_enabled {
                    rpass.set_pipeline(&self.outline_pipeline);
                    rpass.draw(0..world_verts.len() as u32, 0..1);
                }
            }

            // 3. 3D slash crescent trails, traveling dimensional rifts, and impact sparks
            if let Some(ref vb) = vfx_vbuf {
                rpass.set_bind_group(0, &self.camera_bind_group, &[]);
                rpass.set_pipeline(&self.vfx_pipeline);
                rpass.set_vertex_buffer(0, vb.slice(..));
                rpass.draw(0..vfx_verts.len() as u32, 0..1);
            }
        }

        // 4. 2D Sketchbook Vector HUD & Menus pass
        if let Some(ref hb) = hud_vbuf {
            let mut hpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("2D Vector HUD Pass"),
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
            hpass.set_vertex_buffer(0, hb.slice(..));
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
    push_quad(out, Vec3::new(x0, y1, z1), Vec3::new(x1, y1, z1), Vec3::new(x1, y1, z0), Vec3::new(x0, y1, z0), Vec3::Y, color);
    push_quad(out, Vec3::new(x0, y0, z0), Vec3::new(x1, y0, z0), Vec3::new(x1, y0, z1), Vec3::new(x0, y0, z1), Vec3::NEG_Y, color);
    push_quad(out, Vec3::new(x0, y0, z1), Vec3::new(x1, y0, z1), Vec3::new(x1, y1, z1), Vec3::new(x0, y1, z1), Vec3::Z, color);
    push_quad(out, Vec3::new(x1, y0, z0), Vec3::new(x0, y0, z0), Vec3::new(x0, y1, z0), Vec3::new(x1, y1, z0), Vec3::NEG_Z, color);
    push_quad(out, Vec3::new(x1, y0, z1), Vec3::new(x1, y0, z0), Vec3::new(x1, y1, z0), Vec3::new(x1, y1, z1), Vec3::X, color);
    push_quad(out, Vec3::new(x0, y0, z0), Vec3::new(x0, y0, z1), Vec3::new(x0, y1, z1), Vec3::new(x0, y1, z0), Vec3::NEG_X, color);
}

fn push_oriented_box(
    out: &mut Vec<WorldVertex>,
    center: Vec3,
    axis_x: Vec3,
    axis_y: Vec3,
    axis_z: Vec3,
    half: Vec3,
    color: [f32; 4],
) {
    let x = axis_x * half.x;
    let y = axis_y * half.y;
    let z = axis_z * half.z;
    let c000 = center - x - y - z;
    let c100 = center + x - y - z;
    let c110 = center + x + y - z;
    let c010 = center - x + y - z;
    let c001 = center - x - y + z;
    let c101 = center + x - y + z;
    let c111 = center + x + y + z;
    let c011 = center - x + y + z;

    push_quad(out, c011, c111, c110, c010, axis_y, color);
    push_quad(out, c000, c100, c101, c001, -axis_y, color);
    push_quad(out, c001, c101, c111, c011, axis_z, color);
    push_quad(out, c100, c000, c010, c110, -axis_z, color);
    push_quad(out, c101, c100, c110, c111, axis_x, color);
    push_quad(out, c000, c001, c011, c010, -axis_x, color);
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

/// Multi-part animated first-person sword rig (blade + fuller + crossguard + hilt)
/// with windup, fast diagonal/horizontal/overhead follow-through, overshoot, and locomotion sway.
fn push_articulated_sword_rig(
    out: &mut Vec<WorldVertex>,
    camera_pos: Vec3,
    look_dir: Vec3,
    bob_phase: f32,
    locomotion: LocomotionState,
    combat: &CombatState,
    reduced_motion: bool,
) {
    let fwd = look_dir.normalize_or_zero();
    let right = fwd.cross(Vec3::Y).normalize_or_zero();
    let up = right.cross(fwd).normalize_or_zero();

    let sway_x = if reduced_motion { 0.0 } else { bob_phase.cos() * 0.018 };
    let sway_y = if reduced_motion { 0.0 } else { bob_phase.sin().abs() * -0.016 };
    let loco_drop = match locomotion {
        LocomotionState::Sliding => -0.07,
        LocomotionState::Ascending => -0.04,
        LocomotionState::Falling => 0.04,
        _ => 0.0,
    };

    let p = combat.phase_progress();
    let (mut off_r, mut off_u, mut off_f, mut tilt_r, mut tilt_f) = (0.31 + sway_x, -0.25 + sway_y + loco_drop, 0.56, -0.14, 0.16);

    match (combat.state, combat.current_attack) {
        (AttackState::Windup, Some(AttackKind::Heavy)) => {
            off_r += 0.06 * p;
            off_u += 0.22 * p;
            off_f -= 0.08 * p;
            tilt_f -= 0.45 * p;
        }
        (AttackState::Windup, _) => {
            off_r += 0.14 * p;
            off_u += 0.09 * p;
            off_f -= 0.06 * p;
            tilt_r += 0.35 * p;
        }
        (AttackState::Active, Some(AttackKind::LightA)) => {
            // Fast right-to-left diagonal slash
            off_r = 0.44 - 0.88 * p;
            off_u = -0.10 - 0.18 * p;
            off_f = 0.64 + (p * std::f32::consts::PI).sin() * 0.14;
            tilt_r = 0.45 - 1.55 * p;
            tilt_f = 0.55;
        }
        (AttackState::Active, Some(AttackKind::LightB)) => {
            // Reverse left-to-right upward cut
            off_r = -0.40 + 0.86 * p;
            off_u = -0.26 + 0.24 * p;
            off_f = 0.64 + (p * std::f32::consts::PI).sin() * 0.14;
            tilt_r = -1.10 + 1.65 * p;
            tilt_f = 0.50;
        }
        (AttackState::Active, Some(AttackKind::Heavy)) => {
            // Overhead heavy cleave
            off_r = 0.08 - 0.14 * p;
            off_u = 0.18 - 0.56 * p;
            off_f = 0.68 + (p * std::f32::consts::PI).sin() * 0.18;
            tilt_r = -0.15;
            tilt_f = -0.35 + 1.45 * p;
        }
        (AttackState::Active, Some(AttackKind::Dimensional)) => {
            // Wide horizontal reality-rending sweep
            off_r = 0.50 - 1.05 * p;
            off_u = -0.14;
            off_f = 0.72;
            tilt_r = 0.80 - 1.85 * p;
            tilt_f = 0.70;
        }
        (AttackState::Recovery, _) => {
            // Smooth return ease-out
            let inv = 1.0 - p;
            off_r -= 0.26 * inv;
            off_u -= 0.08 * inv;
            tilt_r -= 0.55 * inv;
        }
        (AttackState::Charged, _) => {
            off_f -= 0.10;
            off_u += 0.06;
            tilt_f -= 0.25;
        }
        _ => {
            off_f -= combat.charge_ratio * 0.10;
            off_u += combat.charge_ratio * 0.05;
        }
    }

    let grip_pos = camera_pos + fwd * off_f + right * off_r + up * off_u;
    let blade_up = (up + right * tilt_r + fwd * tilt_f).normalize_or_zero();
    let blade_right = blade_up.cross(fwd).normalize_or_zero();
    let blade_fwd = blade_right.cross(blade_up).normalize_or_zero();

    let blade_col = if combat.charge_ratio > 0.65 || combat.state == AttackState::Charged {
        Palette::ROOF_ORANGE
    } else {
        Palette::PAPER
    };

    // 1. Wrapped charcoal hilt grip
    push_oriented_box(out, grip_pos - blade_up * 0.12, blade_right, blade_up, blade_fwd, Vec3::new(0.018, 0.095, 0.018), Palette::INK);
    // 2. Terracotta-accented crossguard
    push_oriented_box(out, grip_pos, blade_right, blade_up, blade_fwd, Vec3::new(0.085, 0.018, 0.024), Palette::ROOF_ORANGE);
    // 3. Main white sketchbook blade body
    push_oriented_box(out, grip_pos + blade_up * 0.27, blade_right, blade_up, blade_fwd, Vec3::new(0.028, 0.25, 0.010), blade_col);
    // 4. Tapered blade tip
    push_oriented_box(out, grip_pos + blade_up * 0.56, blade_right, blade_up, blade_fwd, Vec3::new(0.016, 0.065, 0.008), blade_col);
    // 5. Charcoal ink fuller ridge along blade center
    push_oriented_box(out, grip_pos + blade_up * 0.24, blade_right, blade_up, blade_fwd, Vec3::new(0.007, 0.20, 0.013), Palette::INK);
}

/// Renders 3D slash crescent ribbons, traveling dimensional rift waves, and impact sparks.
fn push_combat_vfx(out: &mut Vec<WorldVertex>, combat: &CombatState) {
    // 1. Connected white-and-orange slash trail ribbon
    for pair in combat.active_trails.windows(2) {
        let a = &pair[0];
        let b = &pair[1];
        let alpha = (b.remaining_secs / b.max_secs.max(0.01)).clamp(0.0, 1.0) * 0.85;
        let col = if b.is_dimensional {
            [Palette::ROOF_ORANGE[0], Palette::ROOF_ORANGE[1], Palette::ROOF_ORANGE[2], alpha]
        } else {
            [Palette::PAPER[0], Palette::PAPER[1], Palette::PAPER[2], alpha]
        };
        push_quad(out, a.base, a.tip, b.tip, b.base, Vec3::Y, col);
    }

    // 2. Traveling 3D Dimensional Slash crescent wave
    for wave in &combat.active_rifts {
        let half_w = wave.width * 0.5;
        let p0 = wave.position - wave.right * half_w - wave.direction * 0.35;
        let p1 = wave.position + wave.right * half_w - wave.direction * 0.35;
        let p2 = wave.position + wave.right * (half_w * 0.72) + wave.direction * 0.55;
        let p3 = wave.position - wave.right * (half_w * 0.72) + wave.direction * 0.55;
        push_quad(out, p0, p1, p2, p3, Vec3::Y, [Palette::INK[0], Palette::INK[1], Palette::INK[2], 0.92]);
        let lift = Vec3::Y * 0.14;
        push_quad(out, p0 + lift, p1 + lift, p2 + lift, p3 + lift, Vec3::Y, [Palette::ROOF_ORANGE[0], Palette::ROOF_ORANGE[1], Palette::ROOF_ORANGE[2], 0.95]);
    }

    // 3. Impact sparks (orange/charcoal)
    for spark in &combat.active_sparks {
        let s = 0.08;
        let col = if spark.is_orange_accent {
            Palette::ROOF_ORANGE
        } else {
            Palette::INK
        };
        push_box(out, Aabb::from_center_half_extents(spark.position, Vec3::splat(s)), col);
    }
}
