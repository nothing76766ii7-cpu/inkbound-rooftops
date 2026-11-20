// Mobile-compatible directional shadow depth shader.
struct ShadowUniform { light_view_proj: mat4x4<f32>, };
@group(0) @binding(0) var<uniform> shadow: ShadowUniform;
struct Input {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec4<f32>,
    @location(3) material: f32,
};
@vertex
fn vs_shadow(in: Input) -> @builtin(position) vec4<f32> {
    return shadow.light_view_proj * vec4<f32>(in.position, 1.0);
}
