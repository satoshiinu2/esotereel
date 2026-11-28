// core::common — provided by Esotereel lib, not by plugins.
// Cross-package imports require `public` (no keyword = package-visible only).

// Plain clip vertex layout shared by host and shaders. The host type is
// `Vertex` in lib/src/render/vertex.rs (position[3] + color[4] +
// tex_coords[2], tightly packed as 9 x f32). As a WGSL struct this type is
// padded to WGSL alignment, so storage buffers are addressed as
// array<f32> chunks of CLIP_VERTEX_FLOATS instead of arrays of ClipVertex.
public struct ClipVertex {
    position: vec3<f32>,
    color: vec4<f32>,
    tex_coords: vec2<f32>,
}

public const CLIP_VERTEX_FLOATS: u32 = 9u;

// ClipVertex as a vertex shader input: same fields, bound to the
// locations declared by the host vertex buffer layout.
public struct ClipVertexInput {
    @location(0) position: vec3<f32>,
    @location(1) color: vec4<f32>,
    @location(2) tex_coords: vec2<f32>,
}

// Varyings passed from the vertex stage to the fragment stage for a clip.
public struct ClipVertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) tex_coords: vec2<f32>,
}
