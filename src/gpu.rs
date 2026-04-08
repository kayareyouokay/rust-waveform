use crate::audio::{GpuPeak, Waveform};
use bytemuck::{Pod, Zeroable};
use iced::Rectangle;
use iced::advanced::Shell;
use iced::event;
use iced::mouse;
use iced::widget::shader::{Event, Primitive, Program, Storage, Viewport, wgpu};
use std::borrow::Cow;
use std::fmt;
use std::sync::Arc;

const WORKGROUP_SIZE: u32 = 64;
const SUMMARY_TEXTURE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

#[derive(Clone)]
pub struct WaveformProgram<Message> {
    waveform: Arc<Waveform>,
    gain: f32,
    playhead: f32,
    view_start: f32,
    view_end: f32,
    actions: WaveformActions<Message>,
}

#[derive(Clone, Copy)]
pub struct WaveformActions<Message> {
    pub on_seek: fn(f32) -> Message,
    pub on_pan: fn(f32) -> Message,
    pub on_zoom: fn(f32, f32) -> Message,
}

#[derive(Default)]
pub struct WaveformState {
    seeking: bool,
    panning: bool,
    last_pan_x: f32,
}

impl<Message> WaveformProgram<Message> {
    pub fn new(
        waveform: Arc<Waveform>,
        gain: f32,
        playhead: f32,
        view_start: f32,
        view_end: f32,
        actions: WaveformActions<Message>,
    ) -> Self {
        Self {
            waveform,
            gain,
            playhead,
            view_start,
            view_end,
            actions,
        }
    }

    fn local_ratio(bounds: Rectangle, cursor: mouse::Cursor) -> Option<f32> {
        let position = cursor.position_in(bounds)?;
        Some((position.x / bounds.width.max(1.0)).clamp(0.0, 1.0))
    }

    fn local_x(bounds: Rectangle, cursor: mouse::Cursor) -> Option<f32> {
        Some(cursor.position_in(bounds)?.x)
    }
}

impl<Message: 'static> Program<Message> for WaveformProgram<Message> {
    type State = WaveformState;
    type Primitive = WaveformPrimitive;

    fn update(
        &self,
        state: &mut Self::State,
        event: Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
        _shell: &mut Shell<'_, Message>,
    ) -> (event::Status, Option<Message>) {
        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                if let Some(ratio) = Self::local_ratio(bounds, cursor) {
                    state.seeking = true;
                    return (event::Status::Captured, Some((self.actions.on_seek)(ratio)));
                }
            }
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Right)) => {
                if let Some(x) = Self::local_x(bounds, cursor) {
                    state.panning = true;
                    state.last_pan_x = x;
                    return (event::Status::Captured, None);
                }
            }
            Event::Mouse(mouse::Event::CursorMoved { .. }) if state.seeking => {
                if let Some(ratio) = Self::local_ratio(bounds, cursor) {
                    return (event::Status::Captured, Some((self.actions.on_seek)(ratio)));
                }
            }
            Event::Mouse(mouse::Event::CursorMoved { .. }) if state.panning => {
                if let Some(x) = Self::local_x(bounds, cursor) {
                    let delta = x - state.last_pan_x;
                    state.last_pan_x = x;

                    return (
                        event::Status::Captured,
                        Some((self.actions.on_pan)(-(delta / bounds.width.max(1.0)))),
                    );
                }
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                state.seeking = false;
                return (event::Status::Captured, None);
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Right)) => {
                state.panning = false;
                return (event::Status::Captured, None);
            }
            Event::Mouse(mouse::Event::WheelScrolled { delta }) if cursor.is_over(bounds) => {
                let (x, y) = match delta {
                    mouse::ScrollDelta::Lines { x, y } => (x, y),
                    mouse::ScrollDelta::Pixels { x, y } => (x / 40.0, y / 40.0),
                };

                if y.abs() >= x.abs() && y.abs() > 0.0 {
                    if let Some(anchor) = Self::local_ratio(bounds, cursor) {
                        return (
                            event::Status::Captured,
                            Some((self.actions.on_zoom)(anchor, y)),
                        );
                    }
                } else if x.abs() > 0.0 {
                    return (
                        event::Status::Captured,
                        Some((self.actions.on_pan)(x * 0.08)),
                    );
                }
            }
            _ => {}
        }

        (event::Status::Ignored, None)
    }

    fn draw(
        &self,
        _state: &Self::State,
        _cursor: mouse::Cursor,
        _bounds: Rectangle,
    ) -> Self::Primitive {
        WaveformPrimitive {
            waveform: Arc::clone(&self.waveform),
            gain: self.gain.clamp(0.25, 8.0),
            playhead: self.playhead.clamp(0.0, 1.0),
            view_start: self.view_start.clamp(0.0, 1.0),
            view_end: self.view_end.clamp(0.0, 1.0),
        }
    }

    fn mouse_interaction(
        &self,
        state: &Self::State,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if state.panning {
            mouse::Interaction::Grabbing
        } else if cursor.is_over(bounds) {
            mouse::Interaction::Grab
        } else {
            mouse::Interaction::default()
        }
    }
}

#[derive(Clone)]
pub struct WaveformPrimitive {
    waveform: Arc<Waveform>,
    gain: f32,
    playhead: f32,
    view_start: f32,
    view_end: f32,
}

impl fmt::Debug for WaveformPrimitive {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WaveformPrimitive")
            .field("channels", &self.waveform.channels.len())
            .field("peak_bin_count", &self.waveform.peak_bin_count)
            .field("gain", &self.gain)
            .field("playhead", &self.playhead)
            .field("view_start", &self.view_start)
            .field("view_end", &self.view_end)
            .finish()
    }
}

impl Primitive for WaveformPrimitive {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        storage: &mut Storage,
        bounds: &Rectangle,
        viewport: &Viewport,
    ) {
        let scale = viewport.scale_factor() as f32;
        let width = (bounds.width * scale).round().max(1.0) as u32;
        let height = (bounds.height * scale).round().max(1.0) as u32;
        let origin_x = bounds.x * scale;
        let origin_y = bounds.y * scale;

        let needs_rebuild = storage
            .get::<WaveformPipeline>()
            .map(|pipeline| pipeline.needs_rebuild(self, format, width))
            .unwrap_or(true);

        if needs_rebuild {
            storage.store(WaveformPipeline::new(device, queue, self, format, width));
        }

        let Some(pipeline) = storage.get_mut::<WaveformPipeline>() else {
            return;
        };

        let view_start = self.view_start.min(self.view_end);
        let view_end = self.view_end.max(view_start + f32::EPSILON).clamp(0.0, 1.0);

        pipeline.current_columns = width.max(1);
        pipeline.uniforms = GpuUniforms {
            origin_size: [origin_x, origin_y, width as f32, height as f32],
            params: [
                self.playhead,
                self.gain,
                pipeline.current_columns as f32,
                pipeline.channel_count as f32,
            ],
            counts: [
                self.waveform.peak_bin_count as f32,
                view_start,
                view_end,
                0.0,
            ],
        };

        queue.write_buffer(
            &pipeline.uniform_buffer,
            0,
            bytemuck::bytes_of(&pipeline.uniforms),
        );
    }

    fn render(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        storage: &Storage,
        target: &wgpu::TextureView,
        clip_bounds: &Rectangle<u32>,
    ) {
        let Some(pipeline) = storage.get::<WaveformPipeline>() else {
            return;
        };

        let dispatch_count = pipeline
            .current_columns
            .saturating_mul(pipeline.channel_count);
        if dispatch_count == 0 || clip_bounds.width == 0 || clip_bounds.height == 0 {
            return;
        }

        {
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("waveform compute pass"),
                timestamp_writes: None,
            });
            compute_pass.set_pipeline(&pipeline.compute_pipeline);
            compute_pass.set_bind_group(0, &pipeline.compute_bind_group, &[]);
            compute_pass.dispatch_workgroups(dispatch_count.div_ceil(WORKGROUP_SIZE), 1, 1);
        }

        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("waveform render pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
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

        render_pass.set_scissor_rect(
            clip_bounds.x,
            clip_bounds.y,
            clip_bounds.width,
            clip_bounds.height,
        );
        render_pass.set_viewport(
            clip_bounds.x as f32,
            clip_bounds.y as f32,
            clip_bounds.width as f32,
            clip_bounds.height as f32,
            0.0,
            1.0,
        );
        render_pass.set_pipeline(&pipeline.render_pipeline);
        render_pass.set_bind_group(0, &pipeline.render_bind_group, &[]);
        render_pass.draw(0..3, 0..1);
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
struct GpuUniforms {
    origin_size: [f32; 4],
    params: [f32; 4],
    counts: [f32; 4],
}

struct WaveformPipeline {
    waveform_id: usize,
    peak_count: usize,
    format: wgpu::TextureFormat,
    channel_count: u32,
    current_columns: u32,
    uniforms: GpuUniforms,
    _peak_buffer: wgpu::Buffer,
    uniform_buffer: wgpu::Buffer,
    _summary_texture: wgpu::Texture,
    compute_bind_group: wgpu::BindGroup,
    render_bind_group: wgpu::BindGroup,
    compute_pipeline: wgpu::ComputePipeline,
    render_pipeline: wgpu::RenderPipeline,
}

impl WaveformPipeline {
    fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        primitive: &WaveformPrimitive,
        format: wgpu::TextureFormat,
        columns: u32,
    ) -> Self {
        let waveform = &primitive.waveform;
        let current_columns = columns.max(1);
        let channel_count = waveform.channels.len().max(1) as u32;

        let peak_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("waveform peaks"),
            size: (waveform.gpu_peaks.len().max(1) * std::mem::size_of::<GpuPeak>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&peak_buffer, 0, bytemuck::cast_slice(&waveform.gpu_peaks));

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("waveform uniforms"),
            size: std::mem::size_of::<GpuUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let summary_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("waveform summary texture"),
            size: wgpu::Extent3d {
                width: current_columns,
                height: channel_count,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: SUMMARY_TEXTURE_FORMAT,
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let summary_view = summary_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("waveform shader"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(SHADER_SOURCE)),
        });

        let compute_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("waveform compute layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: SUMMARY_TEXTURE_FORMAT,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
            ],
        });

        let render_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("waveform render layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });

        let compute_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("waveform compute bind group"),
            layout: &compute_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: peak_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&summary_view),
                },
            ],
        });

        let render_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("waveform render bind group"),
            layout: &render_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&summary_view),
                },
            ],
        });

        let compute_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("waveform compute pipeline layout"),
                bind_group_layouts: &[&compute_layout],
                push_constant_ranges: &[],
            });

        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("waveform render pipeline layout"),
                bind_group_layouts: &[&render_layout],
                push_constant_ranges: &[],
            });

        let compute_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("waveform compute pipeline"),
            layout: Some(&compute_pipeline_layout),
            module: &shader,
            entry_point: "cs_main",
        });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("waveform render pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        Self {
            waveform_id: Arc::as_ptr(&primitive.waveform) as usize,
            peak_count: waveform.gpu_peaks.len(),
            format,
            channel_count,
            current_columns,
            uniforms: GpuUniforms::default(),
            _peak_buffer: peak_buffer,
            uniform_buffer,
            _summary_texture: summary_texture,
            compute_bind_group,
            render_bind_group,
            compute_pipeline,
            render_pipeline,
        }
    }

    fn needs_rebuild(
        &self,
        primitive: &WaveformPrimitive,
        format: wgpu::TextureFormat,
        columns: u32,
    ) -> bool {
        self.waveform_id != Arc::as_ptr(&primitive.waveform) as usize
            || self.peak_count != primitive.waveform.gpu_peaks.len()
            || self.channel_count != primitive.waveform.channels.len().max(1) as u32
            || self.current_columns != columns.max(1)
            || self.format != format
    }
}

const SHADER_SOURCE: &str = r#"
struct Uniforms {
    origin_size: vec4<f32>,
    params: vec4<f32>,
    counts: vec4<f32>,
}

struct Peak {
    min: f32,
    max: f32,
    avg_abs: f32,
    pad: f32,
}

@group(0) @binding(0)
var<storage, read> peaks: array<Peak>;

@group(0) @binding(1)
var<uniform> uniforms: Uniforms;

@group(0) @binding(2)
var summary_out: texture_storage_2d<rgba16float, write>;

@group(0) @binding(3)
var summary_tex: texture_2d<f32>;

@compute @workgroup_size(64)
fn cs_main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let width = max(u32(uniforms.params.z), 1u);
    let channels = max(u32(uniforms.params.w), 1u);
    let total = width * channels;

    if gid.x >= total {
        return;
    }

    let column = gid.x % width;
    let channel = gid.x / width;
    let peak_count = max(u32(uniforms.counts.x), 1u);
    let view_start_ratio = clamp(uniforms.counts.y, 0.0, 1.0);
    let view_end_ratio = clamp(max(uniforms.counts.z, view_start_ratio), 0.0, 1.0);
    let visible_start = min(u32(floor(view_start_ratio * f32(peak_count))), peak_count - 1u);
    let visible_end = min(
        max(u32(ceil(view_end_ratio * f32(peak_count))), visible_start + 1u),
        peak_count,
    );
    let visible_count = max(visible_end - visible_start, 1u);
    let start = visible_start + (column * visible_count) / width;
    var end = visible_start + ((column + 1u) * visible_count + width - 1u) / width;

    if end <= start {
        end = start + 1u;
    }

    var min_peak = 1.0;
    var max_peak = -1.0;
    var energy = 0.0;
    var count = 0u;
    let offset = channel * peak_count;

    for (var index = start; index < min(end, visible_end); index = index + 1u) {
        let peak = peaks[offset + index];
        min_peak = min(min_peak, peak.min);
        max_peak = max(max_peak, peak.max);
        energy = energy + peak.avg_abs;
        count = count + 1u;
    }

    if count == 0u {
        min_peak = 0.0;
        max_peak = 0.0;
    } else {
        energy = energy / f32(count);
    }

    textureStore(
        summary_out,
        vec2<i32>(i32(column), i32(channel)),
        vec4<f32>(min_peak, max_peak, energy, 1.0),
    );
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var output: VertexOutput;
    var position = vec2<f32>(-1.0, -3.0);

    if (vertex_index == 1u) {
        position = vec2<f32>(-1.0, 1.0);
    } else if (vertex_index == 2u) {
        position = vec2<f32>(3.0, 1.0);
    }

    output.position = vec4<f32>(position, 0.0, 1.0);
    return output;
}

fn mask(distance: f32, width: f32) -> f32 {
    return 1.0 - smoothstep(0.0, width, distance);
}

@fragment
fn fs_main(@builtin(position) frag_position: vec4<f32>) -> @location(0) vec4<f32> {
    let size = max(uniforms.origin_size.zw, vec2<f32>(1.0, 1.0));
    let local = clamp((frag_position.xy - uniforms.origin_size.xy) / size, vec2<f32>(0.0), vec2<f32>(1.0));
    let width = max(u32(uniforms.params.z), 1u);
    let channels = max(u32(uniforms.params.w), 1u);
    let playhead = uniforms.params.x;
    let gain = max(uniforms.params.y, 0.05);
    let view_start = clamp(uniforms.counts.y, 0.0, 1.0);
    let view_end = clamp(max(uniforms.counts.z, view_start + 0.000001), 0.0, 1.0);
    let visible_playhead = (playhead - view_start) / max(view_end - view_start, 0.000001);

    let panel_value = clamp(local.y * f32(channels), 0.0, f32(channels) - 0.001);
    let channel = u32(panel_value);
    let panel_local = fract(panel_value);
    let amplitude = (1.0 - panel_local) * 2.0 - 1.0;

    let column_value = clamp(local.x * f32(width), 0.0, f32(width) - 0.001);
    let column = u32(column_value);
    let stat = textureLoad(summary_tex, vec2<i32>(i32(column), i32(channel)), 0);

    let low = clamp(stat.x * gain, -1.0, 1.0);
    let high = clamp(stat.y * gain, -1.0, 1.0);
    let energy = clamp(stat.z * (0.75 + gain * 0.25), 0.0, 1.0);

    let base_top = vec3<f32>(0.058, 0.102, 0.153);
    let base_bottom = vec3<f32>(0.031, 0.054, 0.094);
    let alternate = vec3<f32>(0.074, 0.121, 0.184);
    let divider = vec3<f32>(0.152, 0.188, 0.250);
    let wave_low = vec3<f32>(0.239, 0.705, 0.776);
    let wave_high = vec3<f32>(0.988, 0.725, 0.333);
    let playhead_color = vec3<f32>(0.969, 0.443, 0.349);

    let panel_mix = select(0.0, 1.0, channel % 2u == 1u);
    var color = mix(base_bottom, mix(base_top, alternate, panel_mix), 1.0 - panel_local);

    let center_line = mask(abs(amplitude), 0.02);
    color = mix(color, divider, center_line * 0.35);

    let waveform_fill = select(0.0, 1.0, amplitude >= low && amplitude <= high);
    let waveform_edge = max(mask(abs(amplitude - low), 0.055), mask(abs(amplitude - high), 0.055));
    let waveform_color = mix(wave_low, wave_high, energy);
    color = mix(color, waveform_color, max(waveform_fill, waveform_edge * 0.45));

    let separator = mask(abs(panel_local), 0.01);
    color = mix(color, divider, separator * 0.75);

    let cursor = select(0.0, mask(abs(local.x - visible_playhead), 2.0 / size.x), visible_playhead >= 0.0 && visible_playhead <= 1.0);
    color = mix(color, playhead_color, cursor);

    return vec4<f32>(color, 1.0);
}
"#;
