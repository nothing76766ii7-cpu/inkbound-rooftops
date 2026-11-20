// Inkbound Rooftops — v2.0 Sketchbook Cel + Roof Tiling + Contact AO + Ink Outlines
// + Speed-Edge Distortion + Dimensional Rift Shader (aligned with reference video)

struct CameraUniform {
    view_proj: mat4x4<f32>,
    camera_pos: vec4<f32>,
    // x = rift_intensity [0..1], y = speed_factor [0..1], z = reduced_flashes (0/1), w = outline_width
    fx_params: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> camera: CameraUniform;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) color: vec4<f32>,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.world_pos = in.position;
    out.world_normal = normalize(in.normal);
    out.color = in.color;
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
    out.clip_position = camera.view_proj * vec4<f32>(extruded, 1.0);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let sun_dir = normalize(vec3<f32>(0.45, 0.82, 0.36));
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
    var rgb = base_rgb * shade + bounce;

    // Airy bright overcast atmospheric depth haze (#EAEBE8)
    let fog_color = vec3<f32>(0.62, 0.66, 0.73);
    let dist = length(in.world_pos - camera.camera_pos.xyz);
    let fog_factor = clamp((dist - 18.0) / 95.0, 0.0, 0.86);
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
    let fog_color = vec3<f32>(0.62, 0.66, 0.73);
    let dist = length(in.world_pos - camera.camera_pos.xyz);
    let fog_factor = clamp((dist - 20.0) / 85.0, 0.0, 0.94);
    let rgb = mix(in.color.rgb, fog_color, fog_factor);
    return vec4<f32>(rgb, 1.0);
}
