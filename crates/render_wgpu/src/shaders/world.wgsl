// Inkbound Rooftops — v2.0 Sketchbook Cel + Roof Tiling + Contact AO + Ink Outlines
// + Speed-Edge Distortion + Dimensional Rift Shader (aligned with reference video)

struct CameraUniform {
    view_proj: mat4x4<f32>,
    camera_pos: vec4<f32>,
    // x = rift_intensity [0..1], y = speed_factor [0..1], z = reduced_flashes (0/1), w = outline_width
    fx_params: vec4<f32>,
    // Sky rgb/daylight, fog rgb/near, sun xyz/fog distance, season tint rgb.
    env_sky: vec4<f32>,
    env_fog: vec4<f32>,
    env_sun: vec4<f32>,
    env_tint: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> camera: CameraUniform;

struct ShadowUniform { light_view_proj: mat4x4<f32>, };
@group(0) @binding(1) var<uniform> shadow: ShadowUniform;
@group(0) @binding(2) var shadow_map: texture_depth_2d;
@group(0) @binding(3) var shadow_sampler: sampler_comparison;
@group(0) @binding(4) var materials: texture_2d<f32>;
@group(0) @binding(5) var material_sampler: sampler;

fn atlas_detail(world: vec3<f32>, normal: vec3<f32>, index: f32) -> vec3<f32> {
    // World-aligned UVs allow arbitrary sliced walls and roofs to retain their texture.
    var projected = world.xz;
    if abs(normal.y) < 0.52 {
        if abs(normal.x) > abs(normal.z) { projected = world.zy; }
        else { projected = world.xy; }
    }
    let quadrant = vec2<f32>(f32(u32(index) % 2u), f32(u32(index) / 2u)) * 0.5;
    let uv = quadrant + vec2<f32>(0.004) + fract(projected * 0.105) * 0.492;
    return textureSampleLevel(materials, material_sampler, uv, 0.0).rgb;
}

fn sun_visibility(world: vec3<f32>, normal: vec3<f32>) -> f32 {
    let clip = shadow.light_view_proj * vec4<f32>(world, 1.0);
    if clip.w <= 0.0 { return 1.0; }
    let uv = vec2<f32>(clip.x / clip.w * 0.5 + 0.5, -clip.y / clip.w * 0.5 + 0.5);
    let depth = clip.z / clip.w;
    if uv.x <= 0.005 || uv.x >= 0.995 || uv.y <= 0.005 || uv.y >= 0.995 || depth <= 0.0 || depth >= 1.0 {
        return 1.0;
    }
    let sun_dir = normalize(camera.env_sun.xyz);
    let bias = max(0.0012, 0.0030 * (1.0 - dot(normal, sun_dir)));
    let texel = 1.0 / 1024.0;
    var lit = 0.0;
    for (var x: i32 = -1; x <= 1; x = x + 1) {
        for (var y: i32 = -1; y <= 1; y = y + 1) {
            lit += textureSampleCompareLevel(shadow_map, shadow_sampler,
                uv + vec2<f32>(f32(x), f32(y)) * texel, depth - bias);
        }
    }
    return 0.23 + 0.77 * (lit / 9.0);
}

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec4<f32>,
    @location(3) material: f32,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) color: vec4<f32>,
    @location(3) @interpolate(flat) material: f32,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.world_pos = in.position;
    out.world_normal = normalize(in.normal);
    out.color = in.color;
    out.material = in.material;
    var clip = camera.view_proj * vec4<f32>(in.position, 1.0);

    // Subtle speed-edge radial stretch (like the "After" reference clip) when sprinting/dashing
    let speed = camera.fx_params.y;
    if (speed > 0.05 && clip.w > 0.01) {
        let ndc = clip.xy / clip.w;
        let radial = dot(ndc, ndc);
        let stretch = 1.0 + speed * 0.065 * clamp(radial - 0.25, 0.0, 1.0);
        clip.x = clip.x * stretch;
        clip.y = clip.y * stretch;
    }

    out.clip_position = clip;
    return out;
}

@vertex
fn vs_outline(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    let dist = length(in.position - camera.camera_pos.xyz);
    // Clean, depth-stabilized charcoal ink contour (avoids heavy black noise in distance)
    let width = camera.fx_params.w * clamp(0.022 + dist * 0.0014, 0.020, 0.11);
    let extruded = in.position + normalize(in.normal) * width;
    out.world_pos = extruded;
    out.world_normal = normalize(in.normal);
    out.color = vec4<f32>(0.145, 0.149, 0.169, 1.0); // Charcoal ink #25262B
    out.material = -1.0;
    out.clip_position = camera.view_proj * vec4<f32>(extruded, 1.0);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let sun_dir = normalize(camera.env_sun.xyz);
    let n = normalize(in.world_normal);
    let ndotl = dot(n, sun_dir);

    // 3-tone painterly sketchbook lighting + subtle warm roof bounce from below
    var shade: f32 = 0.86;
    if (ndotl > 0.32) {
        shade = 1.04;
    } else if (ndotl < -0.10) {
        shade = 0.71;
    }

    var base_rgb = in.color.rgb;
    if in.material >= 0.0 {
        let tex = atlas_detail(in.world_pos, n, in.material);
        // Preserve authored material palette while giving the walls visible stone grain.
        base_rgb *= mix(vec3<f32>(0.65), tex * 1.45, 0.74);
    }

    // Procedural terracotta roof-tile ribbing on warm orange roofs (breaks up flat planes)
    let is_orange_roof = base_rgb.r > 0.78 && base_rgb.g > 0.35 && base_rgb.g < 0.62 && base_rgb.b < 0.35;
    if (is_orange_roof) {
        let tile_u = fract(in.world_pos.x * 0.85 + in.world_pos.z * 0.85);
        let tile_v = fract(in.world_pos.y * 1.45 + in.world_pos.z * 0.55);
        let groove = select(1.0, 0.87, tile_u < 0.09 || tile_v < 0.10);
        base_rgb = base_rgb * groove;
    } else if (base_rgb.r > 0.80 && base_rgb.g > 0.80 && base_rgb.b > 0.78) {
        // Subtle sketchbook plaster tonal variation & vertical contact AO near roof/ground seams
        let plaster_band = 0.96 + 0.04 * sin(in.world_pos.x * 0.6 + in.world_pos.z * 0.45);
        let height_ao = clamp(0.84 + (in.world_pos.y / 28.0) * 0.18, 0.84, 1.02);
        base_rgb = base_rgb * plaster_band * height_ao;
    }

    // Subtle warm terracotta bounce on shadowed vertical walls
    let bounce = vec3<f32>(0.96, 0.72, 0.56) * max(-n.y * 0.10, 0.0);
    var rgb = base_rgb * camera.env_tint.rgb * shade + bounce;
    // Sun shadows shade cut halves, walls and rooftops with a 3x3 soft PCF kernel.
    rgb *= mix(0.55, 1.0, sun_visibility(in.world_pos, n));

    // V8: directional skylight and warm horizon bounce. A continuous diffuse
    // contribution between toon bands prevents the pure-white flat-card look.
    let sky_fill = max(n.y, 0.0) * 0.09;
    let warm_bounce = max(-n.y, 0.0) * 0.055;
    rgb = rgb * (0.92 + sky_fill) + vec3<f32>(0.32, 0.18, 0.10) * warm_bounce;
    // Low-amplitude material grain gives plaster/stone visual depth without
    // texture fetches, important on memory-limited Android devices.
    let grain = fract(sin(dot(floor(in.world_pos.xz * 4.0), vec2<f32>(12.9898, 78.233))) * 43758.5453);
    rgb = rgb * (0.974 + grain * 0.026);

    // Day-night illumination: at midnight cool moonlight replaces harsh diffuse.
    let day = camera.env_sky.w;
    rgb = rgb * (0.24 + 0.76 * day) + vec3<f32>(0.025, 0.045, 0.095) * (1.0-day);
    let fog_color = camera.env_fog.rgb;
    let dist = length(in.world_pos - camera.camera_pos.xyz);
    let fog_factor = clamp((dist - camera.env_fog.w) / max(camera.env_sun.w, 1.0), 0.0, 0.92);
    rgb = mix(rgb, fog_color, fog_factor);

    // Dramatic dimensional slash rift illusion (respects reduced_flashes)
    let rift = camera.fx_params.x;
    let reduced_flashes = camera.fx_params.z;
    if (rift > 0.01 && reduced_flashes < 0.5) {
        let diag = in.clip_position.x * 0.0014 - in.clip_position.y * 0.0030;
        let band = abs(fract(diag) - 0.5);
        if (band < 0.040 * rift) {
            rgb = mix(vec3<f32>(0.145, 0.149, 0.169), vec3<f32>(0.957, 0.482, 0.220), rift);
        } else {
            let luma = dot(rgb, vec3<f32>(0.299, 0.587, 0.114));
            rgb = mix(rgb, vec3<f32>(luma), rift * 0.42);
        }
    }

    return vec4<f32>(rgb, in.color.a);
}

@fragment
fn fs_outline(in: VertexOutput) -> @location(0) vec4<f32> {
    let fog_color = camera.env_fog.rgb;
    let dist = length(in.world_pos - camera.camera_pos.xyz);
    let fog_factor = clamp((dist - camera.env_fog.w) / max(camera.env_sun.w, 1.0), 0.0, 0.94);
    let ink = in.color.rgb * (0.27 + camera.env_sky.w * 0.73);
    let rgb = mix(ink, fog_color, fog_factor);
    return vec4<f32>(rgb, 1.0);
}
