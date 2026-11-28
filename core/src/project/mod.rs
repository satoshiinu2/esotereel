use std::collections::HashMap;
use std::path::Path;

use esotereel_lib::{
    plugin::{
        NamespacedID,
        clip::component::{ComponentCapability, RuntimeDefinition},
        property::{PropertyKey, PropertyPath, value::FieldValue},
    },
    project::{
        Clip, Project, TimelineTick,
        asset::AssetRegistry,
        clip::{ClipBindingValue, get_property_path_from_map, set_property_path_in_map},
        command::{ClipMoveHistoryCtx, ClipResizeHistoryCtx},
        ids::{AssetRef, ClipId, LayerId, TimelineId},
    },
    util::result::EsotereelError,
};

pub(crate) mod commands;
pub(crate) mod history;

/// コマンド実行用に、能力ベースで解決したメディアプロパティのパス一式。
///
/// プロパティ ID をハードコードせず、クリップ種別の `RuntimeDefinition` が
/// 宣言する能力（`FrameProvider` とそのメンバー能力）から解決して実行時に
/// 渡す（AGENTS.md 第9条）。
pub(crate) struct MediaPropertyPaths {
    /// `media-source` メンバーへのパス。クリップ追加時のアセット登録に使う。
    pub source: Option<PropertyPath>,
    /// `media-offset-provider` メンバーへのパス。左エッジリサイズ時のオフセット追随に使う。
    pub offset: Option<PropertyPath>,
}

impl MediaPropertyPaths {
    pub fn resolve(definition: &RuntimeDefinition) -> Self {
        Self {
            source: definition.member_path(
                &ComponentCapability::FrameProvider,
                &ComponentCapability::MediaSource,
            ),
            offset: definition.member_path(
                &ComponentCapability::FrameProvider,
                &ComponentCapability::MediaOffsetProvider,
            ),
        }
    }
}

pub(crate) fn clip_move_mul_core(
    project: &mut Project,
    timeline_id: TimelineId,
    moved_clips: &[ClipMoveHistoryCtx],
) -> anyhow::Result<()> {
    // 移動先レイヤーの存在を、クリップを取り出す前に確認する。
    // 取り出し後に配置が失敗すると、クリップが行き場を失って消えてしまう。
    {
        let timeline = project
            .timeline_ref(timeline_id)
            .ok_or(EsotereelError::TimelineNotFound(timeline_id))?;
        for ctx in moved_clips {
            if timeline.get_layer(ctx.new_layer_id).is_none() {
                return Err(EsotereelError::LayerNotFound(ctx.new_layer_id).into());
            }
        }
    }

    let extracted = {
        let timeline = project
            .timeline_mut(timeline_id)
            .ok_or(EsotereelError::TimelineNotFound(timeline_id))?;

        // (src_layer_id, dest_layer_id, orig_pos, orig_dur, new_pos, new_dur, clip)
        let mut extracted: Vec<(LayerId, LayerId, i64, i64, i64, i64, Clip)> = Vec::new();

        // クリップの取り出し処理
        // remove_clip_by_id が所属レイヤーの検索・除去・Clip実体の取り出しを一括でやってくれる
        for ctx in moved_clips {
            if let Some((clip, src_layer_id)) = timeline.remove_clip_by_id(ctx.clip_id) {
                let original_position = clip.position();
                let original_duration = clip.duration;

                let dest_layer_id = ctx.new_layer_id;
                let new_position = ctx.new_position;
                let new_duration = ctx.new_duration;

                extracted.push((
                    src_layer_id,
                    dest_layer_id,
                    original_position,
                    original_duration,
                    new_position,
                    new_duration,
                    clip,
                ));
            }
        }
        extracted
    };

    // 重なり判定
    let timeline_ref = project
        .timeline_ref(timeline_id)
        .ok_or(EsotereelError::TimelineNotFound(timeline_id))?;

    let exclude_ids: Vec<_> = moved_clips.iter().map(|ctx| ctx.clip_id).collect();
    let has_overlap = extracted
        .iter()
        .any(|(_, dest_layer_id, _, _, new_pos, new_dur, _)| {
            !timeline_ref.can_place_clip_at(*dest_layer_id, *new_pos, *new_dur, &exclude_ids)
        });

    let timeline = project
        .timeline_mut(timeline_id)
        .ok_or(EsotereelError::TimelineNotFound(timeline_id))?;

    if has_overlap {
        // 重なりがある場合は元に戻す
        for (src_layer_id, _dest_layer_id, orig_pos, orig_dur, _new_pos, _new_dur, mut clip) in
            extracted
        {
            clip.set_position(orig_pos);
            clip.duration = orig_dur;
            timeline.place_clip(src_layer_id, clip)?;
        }
        return Ok(());
    }

    // 2. 移動先に配置
    for (src_layer_id, dest_layer_id, _orig_pos, _orig_dur, new_pos, new_dur, mut clip) in extracted
    {
        clip.set_position(new_pos);
        clip.duration = new_dur;
        timeline.place_clip(dest_layer_id, clip)?;
    }

    Ok(())
}

pub(crate) fn clip_resize_core(
    project: &mut Project,
    timeline_id: TimelineId,
    resized_clips: &[ClipResizeHistoryCtx],
    // コンポーネント定義から能力ベースで解決したメディアプロパティのパス。
    // `None` の場合はオフセット調整をスキップする。
    media_paths: Option<&MediaPropertyPaths>,
) -> anyhow::Result<()> {
    // 重なり判定
    {
        let timeline = project
            .timeline_ref(timeline_id)
            .ok_or(EsotereelError::TimelineNotFound(timeline_id))?;
        let exclude_ids: Vec<_> = resized_clips.iter().map(|c| c.clip_id).collect();
        for ctx in resized_clips {
            let (_, layer_id) = timeline
                .get_clip_and_layer(ctx.clip_id)
                .ok_or(EsotereelError::ClipNotFound(ctx.clip_id))?;
            if !timeline.can_place_clip_at(
                layer_id,
                ctx.new_position,
                ctx.new_duration,
                &exclude_ids,
            ) {
                return Ok(());
            }
        }
    }

    let timeline = project
        .timeline_mut(timeline_id)
        .ok_or(EsotereelError::TimelineNotFound(timeline_id))?;

    // クリップの取り出し処理
    let mut extracted = Vec::new();
    for ctx in resized_clips {
        let (clip, layer_id) = timeline
            .remove_clip_by_id(ctx.clip_id)
            .ok_or(EsotereelError::ClipNotFound(ctx.clip_id))?;
        extracted.push((layer_id, ctx, clip));
    }

    // 新しい位置・長さで同じレイヤーに置き直す
    for (layer_id, ctx, mut clip) in extracted {
        clip.set_position(ctx.new_position);
        clip.duration = ctx.new_duration;

        // 左エッジ変更時は source_offset を位置変化分だけシフトする。
        // 左端を右に動かすと source 側からのオフセットも同じだけ進む。
        // プロパティパスはハードコードせず、呼び出し側で能力ベースに解決した
        // ものを受け取って使用する（AGENTS.md 第9条）。
        if ctx.left_edge && ctx.position_delta != 0 {
            if let Some(path) = media_paths.and_then(|paths| paths.offset.as_ref()) {
                if let Some(FieldValue::String(current_offset)) = clip.get_property_path(path) {
                    if let Ok(current) = current_offset.parse::<i64>() {
                        let new_offset = current + ctx.position_delta;
                        let _ = clip.set_property_path(
                            path,
                            ClipBindingValue::Static(FieldValue::String(new_offset.to_string())),
                        );
                    }
                }
            }
        }

        timeline.place_clip(layer_id, clip)?;
    }

    Ok(())
}

/// 文字列で書かれたメディアソースを、レジストリを参照して `AssetRef` 値へ
/// 変換します。
///
/// - 空文字列は未設定（nil UUID）になります。
/// - UUID 形式ならその参照になります。未登録の UUID は存在しないパスとして
///   登録し直したりせず、警告を出して参照のままにします（描画側でも未登録
///   として警告・スキップされます）。
/// - 生パスならレジストリへ登録し、返ってきた参照になります。登録に失敗した
///   場合は警告を出して `None` を返します（呼び出し側は文字列を残します）。
fn legacy_source_string_to_asset_ref(
    assets: &mut AssetRegistry,
    source: &str,
) -> Option<FieldValue> {
    if source.is_empty() {
        return Some(FieldValue::AssetRef(AssetRef::nil()));
    }
    if let Ok(id) = source.parse::<AssetRef>() {
        if assets.get(id).is_none() {
            log::warn!(
                "media source `{source}` is not registered in the project asset registry"
            );
        }
        return Some(FieldValue::AssetRef(id));
    }
    match assets.get_or_add_external(Path::new(source)) {
        Ok(id) => Some(FieldValue::AssetRef(id)),
        Err(error) => {
            log::warn!("failed to register asset `{source}`: {error:#}");
            None
        }
    }
}

/// プロパティマップ内のメディアソースを `AssetRef` 値へ確定させます。
///
/// 既に `AssetRef` 値なら何もしません。文字列なら変換を試み、変換できたら
/// 書き換えて `true` を返します。登録に失敗した場合は文字列のままにして
/// `false` を返します（次にプロジェクトを開いたときの変換で再試行されます）。
/// クリップ作成経路（`clip_add_core`）と、旧形式プロジェクトを開くときの
/// 変換（`crate::save::convert_legacy_media_sources`）の共通処理です。
pub(crate) fn resolve_source_to_asset_ref(
    assets: &mut AssetRegistry,
    properties: &mut HashMap<PropertyKey, ClipBindingValue>,
    source_path: &PropertyPath,
) -> bool {
    let Some(FieldValue::String(source)) =
        get_property_path_from_map(properties, source_path).cloned()
    else {
        return false;
    };
    let Some(value) = legacy_source_string_to_asset_ref(assets, &source) else {
        return false;
    };
    let value = ClipBindingValue::Static(value);
    match set_property_path_in_map(properties, source_path, value) {
        Ok(()) => true,
        Err(error) => {
            log::warn!(
                "failed to store asset reference for media source `{source}`: {error:#}"
            );
            false
        }
    }
}

pub(crate) fn clip_add_core(
    project: &mut Project,
    timeline_id: TimelineId,
    layer_id: LayerId,
    position: TimelineTick,
    duration: TimelineTick,
    kind_id: NamespacedID,
    mut properties: HashMap<PropertyKey, ClipBindingValue>,
    // 能力ベースで解決したメディアプロパティのパス。
    // `None` の場合はアセット登録をスキップする。
    media_paths: Option<&MediaPropertyPaths>,
    preferred_clip_id: Option<ClipId>,
) -> anyhow::Result<ClipId> {
    // クリップのメディアソースをアセットレジストリへ登録し、`source` プロパティを
    // 生パスや UUID 文字列から `AssetRef` 型の値へ確定させる。以後の解決は
    // 描画側がレジストリ経由で行う。ソースへのパスは呼び出し側が能力ベースで
    // 解決したものを使う（AGENTS.md 第9条）。
    if let Some(source_path) = media_paths.and_then(|paths| paths.source.as_ref()) {
        resolve_source_to_asset_ref(project.assets_mut(), &mut properties, source_path);
    }

    if let Some(existing) = preferred_clip_id {
        project.id_generator_mut().observe_clip(existing);
        let timeline = project
            .timeline_mut(timeline_id)
            .ok_or(EsotereelError::TimelineNotFound(timeline_id))?;
        let clip = Clip::new(existing, position, duration, kind_id, properties);
        timeline.place_clip(layer_id, clip)?;
        return Ok(existing);
    }

    let clip_id = project.new_clip_in_timeline(
        timeline_id,
        layer_id,
        position,
        duration,
        kind_id,
        properties,
    )?;

    Ok(clip_id)
}
