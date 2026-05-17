struct Screen {
    size: vec2<f32>,
    _pad: vec2<f32>,
    view_projection: mat4x4<f32>, 
}

@group(0) @binding(0)
var<uniform> screen: Screen;

struct Locals {
    transform: mat4x4<f32>,
}
@group(2) @binding(0)
var<uniform> locals: Locals;

@group(1) @binding(0)
var t_diffuse: texture_2d<f32>;
@group(1) @binding(1)
var s_diffuse: sampler;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) color: vec4<f32>,
    @location(2) tex_coords: vec2<f32>,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) tex_coords: vec2<f32>,
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    // 1. ローカル変換行列を適用
    let world_pos = locals.transform * vec4<f32>(in.position.xyz, 1.0);

    let clip_pos = screen.view_projection * world_pos;

    var out: VertexOutput;
    out.position = clip_pos;
    out.color = in.color;
    out.tex_coords = in.tex_coords;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let tex_color = textureSample(t_diffuse, s_diffuse, in.tex_coords);
    return tex_color * in.color;
}