fn render_frame(ctx, props) {
    let tex = ctx.get_video_texture();
    let transform = props["transform"];
    ctx.render_video(tex, transform.as_mat4());
}
