fn render_clip_video(ctx, props) {
    let tex = ctx.get_video_texture();
    let video = props["std:video"];
    ctx.render_video(tex, video.transform.as_mat4());
}
