fn render_clip_video(ctx, props) {
    ctx.set_transform(mat4_trs(props["std:position"], props["std:rotation"], props["std:scale"]));
    let tex = ctx.get_texture(props["std:source"], ctx.media_time());
    ctx.render_video(tex);
}
