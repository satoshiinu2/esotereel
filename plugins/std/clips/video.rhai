fn render_clip_video(ctx, props) {
    let tex = ctx.get_texture(props.source, 0);
    ctx.render_video(tex, 0.0, 0.0, 1.0, 1.0);
}
