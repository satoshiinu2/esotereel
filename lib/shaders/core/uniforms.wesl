// core::uniforms — provided by Esotereel lib, not by plugins.
// Cross-package imports require `public` (no keyword = package-visible only).

// Frame-common uniform (group 0 of the built-in pipeline). Mirrors the
// host `ScreenUniform` in lib/src/render/uniform.rs: the explicit _pad
// keeps view_projection at the same offset in WGSL and #[repr(C)] layouts.
public struct ScreenUniform {
    size: vec2<f32>,
    _pad: vec2<f32>,
    view_projection: mat4x4<f32>,
}

// Per-batch transform uniform (dynamic-offset binding in the built-in
// pipeline). Mirrors the host transform buffer, one mat4 per batch.
public struct TransformUniform {
    transform: mat4x4<f32>,
}
