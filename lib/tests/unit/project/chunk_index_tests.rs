use super::*;

fn collect(index: &ChunkIndex, range: Range<i64>) -> Vec<(LayerId, ClipId)> {
    index.candidates(range).collect()
}

/// 開始が範囲より前のチャンクにある長いクリップも候補に入ること。
///
/// 以前は範囲の開始・終了が属するチャンクしか走査せず、開始チャンクが
/// 範囲より前にある長いクリップを取りこぼしていた。
#[test]
fn candidates_include_long_clip_starting_in_earlier_chunk() {
    let index = ChunkIndex::build(
        [
            // チャンク 0 に開始し、チャンク 1 の途中まで伸びる長いクリップ。
            (0, 0, 40_000, 1),
            // チャンク 1 に開始する短いクリップ。
            (0, 35_000, 100, 2),
        ]
        .into_iter(),
    );
    let found = collect(&index, 31_000..32_000);
    assert!(found.contains(&(0, 1)), "長いクリップが候補に入るべき: {found:?}");
    assert!(found.contains(&(0, 2)), "範囲内のクリップが候補に入るべき: {found:?}");
}

/// 範囲開始ちょうどで終わるクリップは、遡り走査の対象にならないこと。
#[test]
fn candidates_skip_clip_ending_exactly_at_range_start() {
    let index = ChunkIndex::build([(0, 0, 30_000, 1)].into_iter());
    let found = collect(&index, 30_000..31_000);
    assert!(
        !found.contains(&(0, 1)),
        "範囲開始で終わるクリップは重ならない: {found:?}"
    );
}

/// 範囲内に開始する通常のクリップが候補に入ること。
#[test]
fn candidates_include_clip_starting_inside_range() {
    let index = ChunkIndex::build([(0, 35_000, 100, 2)].into_iter());
    assert_eq!(collect(&index, 30_000..40_000), vec![(0, 2)]);
}
