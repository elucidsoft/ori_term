/// Instance data stride in bytes: 80 bytes per cell instance.
///
/// Layout:
///   [0..8]   pos:      vec2<f32>  (pixel position)
///   [8..16]  size:     vec2<f32>  (pixel size)
///   [16..24] `uv_pos`:   vec2<f32>  (atlas UV top-left)
///   [24..32] `uv_size`:  vec2<f32>  (atlas UV size)
///   [32..48] `fg_color`: vec4<f32>  (foreground RGBA)
///   [48..64] `bg_color`: vec4<f32>  (background RGBA)
///   [64..68] flags:    u32
///   [68..72] `corner_radius`: f32  (0.0 = sharp rect)
///   [72..80] _pad:     8 bytes
pub const INSTANCE_STRIDE: u64 = 80;

/// Instance vertex attributes — shared by both bg and fg pipelines.
const INSTANCE_ATTRS: [wgpu::VertexAttribute; 8] = [
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x2,
        offset: 0,
        shader_location: 0,
    },
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x2,
        offset: 8,
        shader_location: 1,
    },
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x2,
        offset: 16,
        shader_location: 2,
    },
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x2,
        offset: 24,
        shader_location: 3,
    },
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x4,
        offset: 32,
        shader_location: 4,
    },
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x4,
        offset: 48,
        shader_location: 5,
    },
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Uint32,
        offset: 64,
        shader_location: 6,
    },
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32,
        offset: 68,
        shader_location: 7,
    },
];

pub fn instance_buffer_layout() -> wgpu::VertexBufferLayout<'static> {
    wgpu::VertexBufferLayout {
        array_stride: INSTANCE_STRIDE,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &INSTANCE_ATTRS,
    }
}

// --- WGSL Shaders ---

const BG_SHADER_SRC: &str = "
struct Uniforms {
    projection: mat4x4<f32>,
    flags: u32,
    min_contrast: f32,
    _pad: vec2<u32>,
}

@group(0) @binding(0) var<uniform> uniforms: Uniforms;

struct CellInput {
    @location(0) pos: vec2<f32>,
    @location(1) size: vec2<f32>,
    @location(2) uv_pos: vec2<f32>,
    @location(3) uv_size: vec2<f32>,
    @location(4) fg_color: vec4<f32>,
    @location(5) bg_color: vec4<f32>,
    @location(6) flags: u32,
    @location(7) corner_radius: f32,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) bg_color: vec4<f32>,
    @location(1) local_pos: vec2<f32>,
    @location(2) @interpolate(flat) rect_size: vec2<f32>,
    @location(3) @interpolate(flat) radius: f32,
}

@vertex
fn vs_main(@builtin(vertex_index) vi: u32, input: CellInput) -> VertexOutput {
    let corner = vec2<f32>(f32(vi & 1u), f32((vi >> 1u) & 1u));
    let pixel_pos = input.pos + input.size * corner;

    var out: VertexOutput;
    out.position = uniforms.projection * vec4<f32>(pixel_pos, 0.0, 1.0);
    out.bg_color = input.bg_color;
    out.local_pos = corner * input.size;
    out.rect_size = input.size;
    out.radius = input.corner_radius;
    return out;
}

// Iq-style per-corner rounded box SDF.
// p: position relative to box center
// b: box half-size
// r: per-corner radii (topRight, bottomRight, bottomLeft, topLeft)
fn sd_rounded_box(p: vec2<f32>, b: vec2<f32>, r: vec4<f32>) -> f32 {
    var rs = select(r.zw, r.xy, p.x > 0.0);
    rs.x = select(rs.y, rs.x, p.y > 0.0);
    let q = abs(p) - b + rs.x;
    return min(max(q.x, q.y), 0.0) + length(max(q, vec2(0.0))) - rs.x;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    if (input.radius == 0.0) {
        return input.bg_color;
    }

    let half = input.rect_size * 0.5;
    // local_pos is [0..size], remap to centered [-half..+half]
    // y is flipped: top of quad = y=0, but SDF expects top = positive y
    let p = vec2<f32>(
        input.local_pos.x - half.x,
        half.y - input.local_pos.y,
    );

    // Positive radius = top corners only (Chrome-style tabs).
    // Negative radius = all four corners (context menus, popups).
    let abs_r = abs(input.radius);
    var r: vec4<f32>;
    if (input.radius < 0.0) {
        r = vec4<f32>(abs_r, abs_r, abs_r, abs_r);
    } else {
        r = vec4<f32>(abs_r, 0.0, abs_r, 0.0);
    }
    let d = sd_rounded_box(p, half, r);
    let aa = 1.0 - smoothstep(-0.5, 0.5, d);

    if (aa <= 0.0) {
        discard;
    }

    return vec4<f32>(input.bg_color.rgb * aa, input.bg_color.a * aa);
}
";

const FG_SHADER_SRC: &str = "
struct Uniforms {
    projection: mat4x4<f32>,
    flags: u32,
    min_contrast: f32,
    _pad: vec2<u32>,
}

@group(0) @binding(0) var<uniform> uniforms: Uniforms;
@group(1) @binding(0) var glyph_texture: texture_2d<f32>;
@group(1) @binding(1) var glyph_sampler: sampler;

struct CellInput {
    @location(0) pos: vec2<f32>,
    @location(1) size: vec2<f32>,
    @location(2) uv_pos: vec2<f32>,
    @location(3) uv_size: vec2<f32>,
    @location(4) fg_color: vec4<f32>,
    @location(5) bg_color: vec4<f32>,
    @location(6) flags: u32,
    @location(7) corner_radius: f32,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) fg_color: vec4<f32>,
    @location(2) @interpolate(flat) bg_color: vec4<f32>,
}

// --- Color helper functions (ported from Ghostty's GLSL) ---

// sRGB → linear (exact IEC 61966-2-1 transfer)
fn linearize_scalar(v: f32) -> f32 {
    if (v <= 0.04045) {
        return v / 12.92;
    }
    return pow((v + 0.055) / 1.055, 2.4);
}

// linear → sRGB (inverse transfer)
fn unlinearize_scalar(v: f32) -> f32 {
    if (v <= 0.0031308) {
        return v * 12.92;
    }
    return 1.055 * pow(v, 1.0 / 2.4) - 0.055;
}

// ITU-R BT.709 relative luminance (input must be linear)
fn luminance(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
}

// WCAG 2.0 contrast ratio between two linear-space colors
fn contrast_ratio(c1: vec3<f32>, c2: vec3<f32>) -> f32 {
    let l1 = luminance(c1);
    let l2 = luminance(c2);
    let lighter = max(l1, l2);
    let darker = min(l1, l2);
    return (lighter + 0.05) / (darker + 0.05);
}

// Adjust fg to meet minimum contrast against bg.
// If the current contrast is too low, try moving fg toward white or black,
// whichever yields better contrast.
fn contrasted_color(min_ratio: f32, fg: vec4<f32>, bg: vec4<f32>) -> vec4<f32> {
    let ratio = contrast_ratio(fg.rgb, bg.rgb);
    if (ratio >= min_ratio) {
        return fg;
    }

    let bg_l = luminance(bg.rgb);

    // Try both directions: toward white and toward black
    var best = fg;
    var best_ratio = ratio;

    // Binary search toward white
    var lo_w = 0.0;
    var hi_w = 1.0;
    var candidate_w = fg.rgb;
    for (var i = 0; i < 8; i++) {
        let mid = (lo_w + hi_w) / 2.0;
        candidate_w = mix(fg.rgb, vec3<f32>(1.0, 1.0, 1.0), mid);
        let r = contrast_ratio(candidate_w, bg.rgb);
        if (r >= min_ratio) {
            hi_w = mid;
        } else {
            lo_w = mid;
        }
    }
    let white_mix = mix(fg.rgb, vec3<f32>(1.0, 1.0, 1.0), hi_w);
    let white_ratio = contrast_ratio(white_mix, bg.rgb);

    // Binary search toward black
    var lo_b = 0.0;
    var hi_b = 1.0;
    var candidate_b = fg.rgb;
    for (var i = 0; i < 8; i++) {
        let mid = (lo_b + hi_b) / 2.0;
        candidate_b = mix(fg.rgb, vec3<f32>(0.0, 0.0, 0.0), mid);
        let r = contrast_ratio(candidate_b, bg.rgb);
        if (r >= min_ratio) {
            hi_b = mid;
        } else {
            lo_b = mid;
        }
    }
    let black_mix = mix(fg.rgb, vec3<f32>(0.0, 0.0, 0.0), hi_b);
    let black_ratio = contrast_ratio(black_mix, bg.rgb);

    // Pick the direction that achieves the ratio with less color shift
    if (white_ratio >= min_ratio && black_ratio >= min_ratio) {
        // Both work — pick the one with less mix factor (closer to original)
        if (hi_w <= hi_b) {
            return vec4<f32>(white_mix, fg.a);
        } else {
            return vec4<f32>(black_mix, fg.a);
        }
    } else if (white_ratio >= min_ratio) {
        return vec4<f32>(white_mix, fg.a);
    } else if (black_ratio >= min_ratio) {
        return vec4<f32>(black_mix, fg.a);
    }

    // Neither direction fully achieves the target; pick the better one
    if (white_ratio > black_ratio) {
        return vec4<f32>(vec3<f32>(1.0, 1.0, 1.0), fg.a);
    }
    return vec4<f32>(vec3<f32>(0.0, 0.0, 0.0), fg.a);
}

@vertex
fn vs_main(@builtin(vertex_index) vi: u32, input: CellInput) -> VertexOutput {
    let corner = vec2<f32>(f32(vi & 1u), f32((vi >> 1u) & 1u));
    let pixel_pos = input.pos + input.size * corner;

    var out: VertexOutput;
    out.position = uniforms.projection * vec4<f32>(pixel_pos, 0.0, 1.0);
    out.uv = input.uv_pos + input.uv_size * corner;
    out.bg_color = input.bg_color;

    // Minimum contrast enforcement (per-vertex — cheap)
    if (uniforms.min_contrast > 1.0) {
        out.fg_color = contrasted_color(uniforms.min_contrast, input.fg_color, input.bg_color);
    } else {
        out.fg_color = input.fg_color;
    }

    return out;
}

// sRGB render target handles gamma-correct blending automatically:
// the GPU reads the framebuffer in linear, blends in linear, and writes
// back sRGB.  fontdue's area-coverage is already linear, so raw coverage
// is the correct alpha — no manual gamma correction needed.
//
// When linear correction is enabled (flags bit 0), we adjust the alpha
// value to compensate for perceptual luminance differences, producing
// even text weight regardless of fg/bg contrast (Ghostty's approach).

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    var a = textureSample(glyph_texture, glyph_sampler, input.uv).r;
    let color = input.fg_color;

    if ((uniforms.flags & 1u) != 0u) {
        let bg = input.bg_color;
        let fg_l = luminance(color.rgb);
        let bg_l = luminance(bg.rgb);
        if (abs(fg_l - bg_l) > 0.001) {
            // Compute what luminance the naive blend would produce,
            // then solve for alpha that achieves that luminance in
            // sRGB-space blending.
            let blend_l = linearize_scalar(
                unlinearize_scalar(fg_l) * a + unlinearize_scalar(bg_l) * (1.0 - a)
            );
            a = clamp((blend_l - bg_l) / (fg_l - bg_l), 0.0, 1.0);
        }
    }

    // Premultiplied alpha output
    return vec4<f32>(color.rgb * a, a) * color.a;
}
";

// --- Pipeline creation ---

/// Uniform bind group layout: group(0) binding(0) = projection + rendering params.
///
/// Layout (80 bytes):
///   [0..64]  projection: mat4x4<f32>
///   [64..68] flags: u32       (bit 0 = linear alpha correction)
///   [68..72] `min_contrast`: f32 (1.0 = off)
///   [72..80] _padding
pub fn create_uniform_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("uniform_bind_group_layout"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: wgpu::BufferSize::new(80),
            },
            count: None,
        }],
    })
}

/// Atlas texture bind group layout: group(1) binding(0) = texture, binding(1) = sampler.
pub fn create_atlas_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("atlas_bind_group_layout"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    })
}

/// Background pipeline: renders colored quads with premultiplied alpha blending.
pub fn create_bg_pipeline(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    uniform_layout: &wgpu::BindGroupLayout,
    pipeline_cache: Option<&wgpu::PipelineCache>,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("bg_shader"),
        source: wgpu::ShaderSource::Wgsl(BG_SHADER_SRC.into()),
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("bg_pipeline_layout"),
        bind_group_layouts: &[uniform_layout],
        immediate_size: 0,
    });

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("bg_pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[instance_buffer_layout()],
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState {
                    color: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::One,
                        dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                        operation: wgpu::BlendOperation::Add,
                    },
                    alpha: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::One,
                        dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                        operation: wgpu::BlendOperation::Add,
                    },
                }),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleStrip,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: None,
            unclipped_depth: false,
            polygon_mode: wgpu::PolygonMode::Fill,
            conservative: false,
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: pipeline_cache,
    })
}

/// Foreground pipeline: renders alpha-blended textured glyph quads.
pub fn create_fg_pipeline(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    uniform_layout: &wgpu::BindGroupLayout,
    atlas_layout: &wgpu::BindGroupLayout,
    pipeline_cache: Option<&wgpu::PipelineCache>,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("fg_shader"),
        source: wgpu::ShaderSource::Wgsl(FG_SHADER_SRC.into()),
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("fg_pipeline_layout"),
        bind_group_layouts: &[uniform_layout, atlas_layout],
        immediate_size: 0,
    });

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("fg_pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[instance_buffer_layout()],
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState {
                    // Premultiplied alpha: shader outputs (rgb * a, a)
                    color: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::One,
                        dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                        operation: wgpu::BlendOperation::Add,
                    },
                    alpha: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::One,
                        dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                        operation: wgpu::BlendOperation::Add,
                    },
                }),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleStrip,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: None,
            unclipped_depth: false,
            polygon_mode: wgpu::PolygonMode::Fill,
            conservative: false,
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: pipeline_cache,
    })
}
