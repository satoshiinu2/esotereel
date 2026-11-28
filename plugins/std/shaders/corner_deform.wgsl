// Corner Deform (Compute): クワッドの四隅だけをずらす変形を compute パスで
// 頂点に適用するシェーダ。
//
// 各頂点のテクスチャ座標をクワッド内の正規化座標 (u, v) とみなし、四隅
// オフセットの双線形補間を position の x/y に加える。隅の頂点は対応する
// オフセットそのものだけ移動し、内側は滑らかに補間される。頂点レイアウトと
// ping-pong の決まりは wave_compute.wgsl の冒頭コメントを参照。

import core::common::CLIP_VERTEX_FLOATS;

struct Params {
    top_left_x: f32,
    top_left_y: f32,
    top_right_x: f32,
    top_right_y: f32,
    bottom_right_x: f32,
    bottom_right_y: f32,
    bottom_left_x: f32,
    bottom_left_y: f32,
}

@group(0) @binding(0)
var<uniform> params: Params;

@group(1) @binding(0)
var<storage, read> vertices_in: array<f32>;
@group(1) @binding(1)
var<storage, read_write> vertices_out: array<f32>;

@compute @workgroup_size(64)
fn deform_main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if (i >= arrayLength(&vertices_in) / CLIP_VERTEX_FLOATS) {
        return;
    }

    let base = i * CLIP_VERTEX_FLOATS;

    for (var k: u32 = 0u; k < CLIP_VERTEX_FLOATS; k++) {
        vertices_out[base + k] = vertices_in[base + k];
    }

    // tex_coords は position[3] + color[4] の後に置かれる（オフセット 7, 8）。
    let u = vertices_in[base + 7u];
    let v = vertices_in[base + 8u];

    let top = mix(
        vec2<f32>(params.top_left_x, params.top_left_y),
        vec2<f32>(params.top_right_x, params.top_right_y),
        u,
    );
    let bottom = mix(
        vec2<f32>(params.bottom_left_x, params.bottom_left_y),
        vec2<f32>(params.bottom_right_x, params.bottom_right_y),
        u,
    );
    let offset = mix(top, bottom, v);

    vertices_out[base] = vertices_in[base] + offset.x;
    vertices_out[base + 1u] = vertices_in[base + 1u] + offset.y;
}
