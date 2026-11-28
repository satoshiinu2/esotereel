import core::common::{ClipVertexInput, ClipVertexOutput};
import core::uniforms::{ScreenUniform, TransformUniform};

@group(0) @binding(0)
var<uniform> screen: ScreenUniform;

@group(2) @binding(0)
var<uniform> locals: TransformUniform;

@group(1) @binding(0)
var t_diffuse: texture_2d<f32>;
@group(1) @binding(1)
var s_diffuse: sampler;

@vertex
fn vs_main(in: ClipVertexInput) -> ClipVertexOutput {
    // 1. ローカル変換行列を適用
    let world_pos = locals.transform * vec4<f32>(in.position.xyz, 1.0);

    let clip_pos = screen.view_projection * world_pos;

    var out: ClipVertexOutput;
    out.position = clip_pos;
    out.color = in.color;
    out.tex_coords = in.tex_coords;
    return out;
}

@fragment
fn fs_main(in: ClipVertexOutput) -> @location(0) vec4<f32> {
    let tex_color = textureSample(t_diffuse, s_diffuse, in.tex_coords);
    return tex_color * in.color;
}
