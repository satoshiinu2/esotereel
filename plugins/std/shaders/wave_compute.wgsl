// Wave (Compute): 正弦波変形を、頂点シェーダではなく compute パスで
// storage buffer 上の頂点に適用するシェーダ。
//
// 使い方:
// - 頂点データは storage buffer に置く。シェーダ1つ = compute パス1回。
// - 入力は常に「元のバッファ」。lib がパスごとに in / out のバインドを
//   入れ替えて ping-pong し、最後の出力バッファを普通のパイプラインで描く。
//   毎フレーム変形を累積させないための決まりで、入力に出力バッファを
//   そのまま渡してはいけない。
// - 描画側は変形なしの頂点シェーダ（今の shape.wgsl 相当）を使う。
//   頂点シェーダ側でも変形するシェーダと組み合わせると二重にかかるので注意。
// - 頂点シェーダと違い、隣の頂点を読める。例えば
//   `vertices_in[base - 9u]` / `vertices_in[base + 9u]` で前後の頂点を
//   参照でき、平滑化や揺れの伝播のような変形が書ける。
//
// 頂点レイアウトの注意:
// lib の Vertex は #[repr(C)] で position[3] + color[4] + tex_coords[2]
// = 36 バイト（lib/src/render/vertex.rs）。WGSL の vec3<f32> は
// 16 バイトアラインのため struct にすると 48 バイトになって合わない。
// そのためここでは array<f32> を CLIP_VERTEX_FLOATS 個ずつで扱う。
// 頂点定義そのものは core::common（lib 提供モジュール）から import する。

import core::common::CLIP_VERTEX_FLOATS;

struct Params {
    time: f32,
    amplitude: f32,
    frequency: f32,
    speed: f32,
}

@group(0) @binding(0)
var<uniform> params: Params;

// @group(1): ping-pong する頂点バッファ。in は read 専用、out は read_write。
@group(1) @binding(0)
var<storage, read> vertices_in: array<f32>;
@group(1) @binding(1)
var<storage, read_write> vertices_out: array<f32>;

@compute @workgroup_size(64)
fn deform_main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    // dispatch 数は lib が ceil(頂点数 / 64) で決める。余りはここで弾く。
    if (i >= arrayLength(&vertices_in) / CLIP_VERTEX_FLOATS) {
        return;
    }

    let base = i * CLIP_VERTEX_FLOATS;

    // 頂点全体をコピーしてから、position.y だけ書き換える。
    for (var k: u32 = 0u; k < CLIP_VERTEX_FLOATS; k++) {
        vertices_out[base + k] = vertices_in[base + k];
    }

    let x = vertices_in[base];
    let y = vertices_in[base + 1u];
    vertices_out[base + 1u] = y + sin(x * params.frequency + params.time * params.speed) * params.amplitude;
}
