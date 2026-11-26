use super::*;

use std::collections::HashMap;

use crate::plugin::NamespacedID;
use crate::project::ids::IdGenerator;

fn kind(name: &str) -> NamespacedID {
    NamespacedID::parse(name).unwrap()
}

/// レイヤーを1つ持つ空のタイムラインと、そのレイヤー ID を返します。
fn single_layer_timeline() -> (Timeline, IdGenerator, LayerId) {
    let mut ids = IdGenerator::default();
    let mut timeline = Timeline::new_empty(1, 60.0);
    let layer = timeline
        .insert_layer(&mut ids, "Layer 1".into(), None, None)
        .unwrap();
    (timeline, ids, layer)
}

fn add_clip(
    timeline: &mut Timeline,
    ids: &mut IdGenerator,
    layer: LayerId,
    position: i64,
    duration: i64,
    kind_id: &str,
) -> ClipId {
    timeline
        .new_clip_in(layer, ids, position, duration, kind(kind_id), HashMap::new())
        .unwrap()
}

fn query_clip_ids(timeline: &Timeline, range: Range<i64>) -> Vec<ClipId> {
    timeline
        .query_range(range)
        .iter()
        .map(|(_, clip)| clip.id)
        .collect()
}

/// 開始が範囲より前のチャンクにある長いクリップも範囲検索で拾うこと。
///
/// 旧 ChunkIndex の candidates に対する回帰テストの移し替えです
/// （チャンク所有化で検索経路が Timeline::query_range に移ったため、
/// 観測可能な振る舞いとしてこちらで担保します）。
#[test]
fn query_range_includes_long_clip_starting_in_earlier_chunk() {
    let (mut timeline, mut ids, layer) = single_layer_timeline();
    let layer2 = timeline
        .insert_layer(&mut ids, "Layer 2".into(), None, None)
        .unwrap();
    // チャンク 0 に開始し、チャンク 1 の途中まで伸びる長いクリップ。
    let long = add_clip(&mut timeline, &mut ids, layer, 0, 40_000, "std:media");
    // チャンク 1 に開始する短いクリップ（長いクリップと重なるため別レイヤー）。
    let short = add_clip(&mut timeline, &mut ids, layer2, 35_000, 100, "std:media");

    // 旧 ChunkIndex テストは候補列挙レベルの検証だったが、query_range は
    // 厳密に重なるものだけを返すため、両方と重なる範囲で確認する。
    let found = query_clip_ids(&timeline, 31_000..36_000);
    assert!(found.contains(&long), "長いクリップが含まれるべき: {found:?}");
    assert!(found.contains(&short), "範囲内のクリップが含まれるべき: {found:?}");
}

/// 範囲開始ちょうどで終わるクリップは範囲検索に含まれないこと。
#[test]
fn query_range_skips_clip_ending_exactly_at_range_start() {
    let (mut timeline, mut ids, layer) = single_layer_timeline();
    add_clip(&mut timeline, &mut ids, layer, 0, 30_000, "std:media");

    let found = query_clip_ids(&timeline, 30_000..31_000);
    assert!(found.is_empty(), "範囲開始で終わるクリップは重ならない: {found:?}");
}

/// 範囲内に開始する通常のクリップが範囲検索に含まれること。
#[test]
fn query_range_includes_clip_starting_inside_range() {
    let (mut timeline, mut ids, layer) = single_layer_timeline();
    let clip = add_clip(&mut timeline, &mut ids, layer, 35_000, 100, "std:media");

    assert_eq!(query_clip_ids(&timeline, 30_000..40_000), vec![clip]);
}

/// 配置は所属レイヤー・duration・kind 連番を持ち、同じ kind は同じ連番を
/// 共有し、連番から文字列へ戻せること。
#[test]
fn placement_tracks_layer_duration_and_interned_kind() {
    let (mut timeline, mut ids, layer) = single_layer_timeline();
    let layer2 = timeline
        .insert_layer(&mut ids, "Layer 2".into(), None, None)
        .unwrap();
    let a = add_clip(&mut timeline, &mut ids, layer, 0, 100, "std:media");
    let b = add_clip(&mut timeline, &mut ids, layer2, 0, 200, "std:media");
    let c = add_clip(&mut timeline, &mut ids, layer2, 300, 100, "std:transform");

    let pa = timeline.get_placement(a).unwrap();
    assert_eq!(pa.layer_id, layer);
    assert_eq!(pa.duration, 100);

    let pb = timeline.get_placement(b).unwrap();
    assert_eq!(pb.layer_id, layer2);
    assert_eq!(pb.kind, pa.kind, "同じ kind は同じ連番を共有する");

    let pc = timeline.get_placement(c).unwrap();
    assert_ne!(pc.kind, pa.kind, "異なる kind は異なる連番になる");
    assert_eq!(
        timeline.kind_table().resolve(pc.kind).unwrap().full(),
        "std:transform"
    );
    assert_eq!(timeline.layer_of_clip(c), Some(layer2));
}

/// kind の連番は、クリップが消えても再利用されないこと（append-only）。
#[test]
fn kind_numbers_are_not_reused_after_clip_removal() {
    let (mut timeline, mut ids, layer) = single_layer_timeline();
    let a = add_clip(&mut timeline, &mut ids, layer, 0, 100, "std:media");
    let b = add_clip(&mut timeline, &mut ids, layer, 200, 100, "std:transform");
    let kind_a = timeline.get_placement(a).unwrap().kind;
    let kind_b = timeline.get_placement(b).unwrap().kind;

    timeline.remove_clip_by_id(b).unwrap();

    let c = add_clip(&mut timeline, &mut ids, layer, 400, 100, "std:audio");
    let kind_c = timeline.get_placement(c).unwrap().kind;
    assert_ne!(kind_c, kind_b, "消えた kind の番号を再利用しない");
    assert_ne!(kind_c, kind_a);
    // 欠番になった連番も表には残り、解決できる。
    assert_eq!(
        timeline.kind_table().resolve(kind_b).unwrap().full(),
        "std:transform"
    );
}

/// チャンクをまたぐ move_clip の後も、実体と配置が一致し続けること。
#[test]
fn move_clip_across_chunks_keeps_body_and_placement_consistent() {
    let (mut timeline, mut ids, layer) = single_layer_timeline();
    let clip = add_clip(&mut timeline, &mut ids, layer, 0, 100, "std:media");

    timeline.move_clip(clip, 35_000).unwrap();

    assert_eq!(timeline.get_clip(clip).unwrap().position, 35_000);
    let placement = timeline.get_placement(clip).unwrap();
    assert_eq!(placement.position, 35_000);
    assert_eq!(placement.layer_id, layer);
    assert_eq!(query_clip_ids(&timeline, 30_000..40_000), vec![clip]);
    assert!(query_clip_ids(&timeline, 0..100).is_empty());
}

/// remove_clip_by_id は所属レイヤーを配置から引き、正しいレイヤーを返すこと。
#[test]
fn remove_clip_by_id_returns_owning_layer() {
    let (mut timeline, mut ids, layer) = single_layer_timeline();
    let layer2 = timeline
        .insert_layer(&mut ids, "Layer 2".into(), None, None)
        .unwrap();
    add_clip(&mut timeline, &mut ids, layer, 0, 100, "std:media");
    let target = add_clip(&mut timeline, &mut ids, layer2, 0, 100, "std:media");

    let (removed, owner) = timeline.remove_clip_by_id(target).unwrap();
    assert_eq!(removed.id, target);
    assert_eq!(owner, layer2);
    assert!(timeline.get_clip(target).is_none());
    assert!(timeline.get_placement(target).is_none());
}
