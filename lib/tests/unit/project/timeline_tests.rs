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

use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::project::chunk::{ChunkResidencyConfig, ChunkState, ChunkStore};
use crate::util::result::EsotereelResult;

/// テスト用のインメモリ ChunkStore です。本体はテスト側からも触れるよう
/// Arc で共有します（配置と食い違う実体を返す検証のため）。
#[derive(Debug, Default)]
struct MemoryStore {
    chunks: Arc<Mutex<HashMap<ChunkId, Vec<(LayerId, Clip)>>>>,
}

impl ChunkStore for MemoryStore {
    fn load_chunk(&mut self, chunk_id: ChunkId) -> EsotereelResult<Vec<(LayerId, Clip)>> {
        Ok(self
            .chunks
            .lock()
            .unwrap()
            .get(&chunk_id)
            .cloned()
            .unwrap_or_default())
    }

    fn store_chunk(
        &mut self,
        chunk_id: ChunkId,
        clips: &[(LayerId, Clip)],
    ) -> EsotereelResult<()> {
        self.chunks
            .lock()
            .unwrap()
            .insert(chunk_id, clips.to_vec());
        Ok(())
    }
}

/// アンロードすると実体は引けなくなるが配置は残り、読み込み直せば
/// 実体が戻ること。
#[test]
fn unload_drops_body_but_keeps_placement_and_reload_restores_it() {
    let (mut timeline, mut ids, layer) = single_layer_timeline();
    let clip = add_clip(&mut timeline, &mut ids, layer, 0, 100, "std:media");
    timeline.set_chunk_store(Some(Box::new(MemoryStore::default())));

    timeline.unload_chunk(0).unwrap();
    assert_eq!(timeline.chunk_state(0), Some(ChunkState::Unloaded));
    assert!(timeline.get_clip(clip).is_none(), "実体は落ちている");
    assert!(
        timeline.get_placement(clip).is_some(),
        "配置は常駐したまま引ける"
    );
    assert_eq!(
        timeline.query_placements(0..100).len(),
        1,
        "配置検索にはアンロード中も現れる"
    );
    assert!(
        query_clip_ids(&timeline, 0..100).is_empty(),
        "実体検索にはアンロード中は現れない"
    );

    timeline.ensure_clip_loaded(clip).unwrap();
    assert_eq!(timeline.chunk_state(0), Some(ChunkState::Loaded));
    let restored = timeline.get_clip(clip).unwrap();
    assert_eq!(restored.position, 0);
    assert_eq!(restored.duration, 100);
}

/// アンロード中のクリップへの変更操作は、先に読み込んでから受け付けること。
#[test]
fn get_clip_mut_loads_unloaded_chunk_first() {
    let (mut timeline, mut ids, layer) = single_layer_timeline();
    let clip = add_clip(&mut timeline, &mut ids, layer, 0, 100, "std:media");
    timeline.set_chunk_store(Some(Box::new(MemoryStore::default())));
    timeline.unload_chunk(0).unwrap();
    assert!(!timeline.is_clip_loaded(clip));

    let body = timeline.get_clip_mut(clip).expect("読み込んでから返る");
    assert_eq!(body.id, clip);
    assert!(timeline.is_clip_loaded(clip));
}

/// ストアが無い状態でのアンロードは、黙って実体を捨てずエラーになること。
#[test]
fn unload_without_store_fails_and_keeps_body() {
    let (mut timeline, mut ids, layer) = single_layer_timeline();
    let clip = add_clip(&mut timeline, &mut ids, layer, 0, 100, "std:media");

    assert!(timeline.unload_chunk(0).is_err());
    assert_eq!(timeline.chunk_state(0), Some(ChunkState::Loaded));
    assert!(timeline.get_clip(clip).is_some());
}

/// ストアが配置と食い違う実体を返したら、取り込まずエラーにし、
/// チャンクはアンロード状態に戻ること。
#[test]
fn load_rejects_bodies_that_disagree_with_placements() {
    let (mut timeline, mut ids, layer) = single_layer_timeline();
    let clip = add_clip(&mut timeline, &mut ids, layer, 0, 100, "std:media");
    let store = MemoryStore::default();
    let shared = store.chunks.clone();
    timeline.set_chunk_store(Some(Box::new(store)));
    timeline.unload_chunk(0).unwrap();

    // 配置（position 0）と食い違う実体に差し替える。
    {
        let mut chunks = shared.lock().unwrap();
        let bodies = chunks.get_mut(&0).unwrap();
        bodies[0].1.position = 5;
    }

    assert!(timeline.ensure_clip_loaded(clip).is_err());
    assert_eq!(timeline.chunk_state(0), Some(ChunkState::Unloaded));
    assert!(timeline.get_clip(clip).is_none());
}

/// unload_idle_chunks は、未使用時間が閾値を超えたチャンクだけを落とし、
/// 頻繁に使われたチャンクは保持すること。
#[test]
fn unload_idle_chunks_respects_idle_time_and_frequency() {
    let (mut timeline, mut ids, layer) = single_layer_timeline();
    // チャンク 0 と チャンク 1 に1つずつ置く。
    let hot = add_clip(&mut timeline, &mut ids, layer, 0, 100, "std:media");
    let cold = add_clip(&mut timeline, &mut ids, layer, 35_000, 100, "std:media");
    timeline.set_chunk_store(Some(Box::new(MemoryStore::default())));

    // hot 側は繰り返し触る（登録時の1回＋ここで3回 = 4アクセス）。
    for _ in 0..3 {
        timeline.get_clip(hot).unwrap();
    }

    let config = ChunkResidencyConfig {
        idle_timeout: Duration::ZERO,
        frequent_access_threshold: 4,
    };
    let unloaded = timeline.unload_idle_chunks(&config).unwrap();
    assert_eq!(unloaded, 1);
    assert_eq!(timeline.chunk_state(0), Some(ChunkState::Loaded));
    assert_eq!(timeline.chunk_state(1), Some(ChunkState::Unloaded));
    assert!(timeline.get_clip(hot).is_some());
    assert!(timeline.get_clip(cold).is_none());

    // 閾値が大きい間は、時間が経っていなければ何も落ちない。
    let patient = ChunkResidencyConfig {
        idle_timeout: Duration::from_secs(3600),
        frequent_access_threshold: 0,
    };
    assert_eq!(timeline.unload_idle_chunks(&patient).unwrap(), 0);
    assert_eq!(timeline.chunk_state(0), Some(ChunkState::Loaded));
}
