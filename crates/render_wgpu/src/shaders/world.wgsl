// Inkbound Rooftops — Stylized Sketchbook Cel + Ink Outline + Overcast Fog + Dimensional Rift Shader

struct CameraUniform {
    view_proj: mat4x4<f32>,
    camera_pos: vec4<f32>,
    // x = rift_intensity [0..1], y = rift_angle, z = reduced_flashes (0 or 1), w = outline_width
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
    out.clip_position = camera.view_proj * vec4<f32>(in.position, 1.0);
    return out;
}

@vertex
fn vs_outline(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    let dist = length(in.position - camera.camera_pos.xyz);
    // Scale charcoal outline slightly with distance, capped for mobile readability.
    let width = camera.fx_params.w * clamp(0.028 + dist * 0.0018, 0.025, 0.14);
    let extruded = in.position + normalize(in.normal) * width;
    out.world_pos = extruded;
    out.world_normal = normalize(in.normal);
    out.color = vec4<f32>(0.145, 0.149, 0.169, 1.0); // Charcoal ink #25262B
    out.clip_position = camera.view_proj * vec4<f32>(extruded, 1.0);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let sun_dir = normalize(vec3<f32>(0.42, 0.84, 0.34));
    let n = normalize(in.world_normal);
    let ndotl = dot(n, sun_dir);

    // 2-band sketchbook cel shading with bright overcast fill
    var shade: f32 = 0.84;
    if (ndotl > 0.25) {
        shade = 1.02;
    } else if (ndotl < -0.15) {
        shade = 0.72;
    }

    var rgb = in.color.rgb * shade;

    // Subtle washed-out white/gray overcast atmospheric fog (#E8E9E7)
    let fog_color = vec3<f32>(0.910, 0.914, 0.906);
    let dist = length(in.world_pos - camera.camera_pos.xyz);
    let fog_factor = clamp((dist - 28.0) / 115.0, 0.0, 0.85);
    rgb = mix(rgb, fog_color, fog_factor);

    // Dramatic dimensional slash rift illusion (screen-aligned ink cut without altering building colliders)
    let rift = camera.fx_params.x;
    let reduced_flashes = camera.fx_params.z;
    if (rift > 0.01 && reduced_flashes < 0.5) {
        let diag = in.clip_position.x * 0.0015 - in.clip_position.y * 0.0032;
        let band = abs(fract(diag) - 0.5);
        if (band < 0.035 * rift) {
            // Sharp charcoal-and-orange spatial cut ribbon
            rgb = mix(vec3<f32>(0.145, 0.149, 0.169), vec3<f32>(0.957, 0.482, 0.220), rift);
        } else {
            // Brief monochrome sketch wash during dimensional slash
            let luma = dot(rgb, vec3<f32>(0.299, 0.587, 0.114));
            rgb = mix(rgb, vec3<f32>(luma), rift * 0.45);
        }
    }

    return vec4<f32>(rgb, in.color.a);
}

@fragment
fn fs_outline(in: VertexOutput) -> @location(0) vec4<f32> {
    let fog_color = vec3<f32>(0.910, 0.914, 0.906);
    let dist = length(in.world_pos - camera.camera_pos.xyz);
    let fog_factor = clamp((dist - 24.0) / 90.0, 0.0, 0.92);
    let rgb = mix(in.color.rgb, fog_color, fog_factor);
    return vec4<f32>(rgb, 1.0);
}
