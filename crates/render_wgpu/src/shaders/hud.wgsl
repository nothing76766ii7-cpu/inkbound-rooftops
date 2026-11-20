// 2D Sketchbook HUD & Menu Overlay Shader

struct HudVertexInput {
    @location(0) position: vec2<f32>,
    @location(1) color: vec4<f32>,
};

struct HudVertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_hud(in: HudVertexInput) -> HudVertexOutput {
    var out: HudVertexOutput;
    // Convert normalized [0, 1] top-left screen coordinates to NDC [-1, 1].
    let ndc_x = in.position.x * 2.0 - 1.0;
    let ndc_y = 1.0 - in.position.y * 2.0;
    out.clip_position = vec4<f32>(ndc_x, ndc_y, 0.0, 1.0);
    out.color = in.color;
    return out;
}

@fragment
fn fs_hud(in: HudVertexOutput) -> @location(0) vec4<f32> {
    return in.color;
}
