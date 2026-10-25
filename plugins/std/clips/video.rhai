fn render_clip_video(ctx, props) {
    let video = props["std:video"];
    let transform = video.transform;
    ctx.set_transform(mat4_trs(transform.position, transform.rotation, transform.scale));
    let tex = ctx.get_texture(video.media.source, ctx.media_time());
    ctx.render_video(tex);
}
