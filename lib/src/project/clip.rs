use rkyv::{Archive, CheckBytes, bytecheck};
use std::collections::{BTreeMap, HashMap, HashSet};

use crate::{
    plugin::{
        NamespacedID,
        property::{PropertyKey, PropertyPath, PropertySchema, value::FieldValue},
    },
    project::{
        TimelineTick,
        ids::{ClipId, TimelineId},
    },
};

/// クリップに適用する compute シェーダ 1 件分のスタックエントリです。
///
/// `shader_id` でシェーダ定義（例: `std:wave_compute`）を参照し、`properties` に
/// そのシェーダ固有プロパティの値を保持します。キーはシェーダ定義側で宣言された
/// ローカルキーで、`pack_params` の固有プロパティ値としてそのまま渡されます。
/// 値が無いプロパティはシェーダ定義の `default` が使われます。
///
/// スタックはクリップ種別やコンポーネントのプロパティではなく lib が所有する
/// 汎用データであり、特定プラグインのプロパティ ID には依存しません。
#[derive(
    Archive,
    rkyv::Deserialize,
    rkyv::Serialize,
    serde::Serialize,
    serde::Deserialize,
    Debug,
    Clone,
    PartialEq,
)]
#[archive_attr(derive(CheckBytes))]
pub struct ShaderStackEntry {
    pub shader_id: NamespacedID,
    pub properties: HashMap<String, FieldValue>,
}

/// タイムライン上のクリップ実体です。
///
/// `id` で識別され、`position`/`duration` で配置範囲を持ち、`kind_id` で
/// クリップ種別（`NamespacedID`）を参照します。プロパティ値は
/// `properties` マップに `ClipBindingValue` として保持されます。
///
/// `shader_stack` はクリップへ適用する compute シェーダ（エフェクト）の
/// 順序付きリストで、描画時に先頭から順に頂点へ適用されます。
///
/// Clip 実体は `Timeline` のチャンクが所有し、`Layer.clips` は位置 → `ClipId`
/// の参照のみを保持します。
#[derive(
    Archive, rkyv::Deserialize, rkyv::Serialize, serde::Serialize, serde::Deserialize, Debug, Clone,
)]
#[archive_attr(derive(CheckBytes))]
pub struct Clip {
    pub id: ClipId,
    pub position: TimelineTick,
    pub duration: TimelineTick,

    pub kind_id: NamespacedID,
    pub properties: HashMap<PropertyKey, ClipBindingValue>,

    /// 適用する compute シェーダのスタック。空なら変形なし（従来どおり描画）。
    /// 旧形式の保存データには存在しないため、serde では空リストを既定とします。
    #[serde(default)]
    pub shader_stack: Vec<ShaderStackEntry>,
}

impl Clip {
    /// 新しい `Clip` を作成します。
    pub fn new(
        id: ClipId,
        position: TimelineTick,
        duration: TimelineTick,
        kind_id: NamespacedID,
        properties: HashMap<PropertyKey, ClipBindingValue>,
    ) -> Self {
        Self {
            id,
            position,
            duration,
            kind_id,
            properties,
            shader_stack: Vec::new(),
        }
    }

    /// シェーダスタック（適用順）への参照を返します。
    pub fn shader_stack(&self) -> &[ShaderStackEntry] {
        &self.shader_stack
    }

    /// シェーダスタックを丸ごと差し替えます。
    pub fn set_shader_stack(&mut self, stack: Vec<ShaderStackEntry>) {
        self.shader_stack = stack;
    }

    /// 配置開始位置（タイムラインティック）を返します。
    pub fn position(&self) -> i64 {
        self.position
    }

    /// 配置開始位置を変更します。
    ///
    /// `position`/`duration` を変更する場合は `Timeline::move_clip` を経由することで、
    /// `layer.clips` と配置の整合性を保ちます。
    pub fn set_position(&mut self, new_pos: i64) {
        self.position = new_pos;
    }

    /// 指定キーのプロパティ値を返します。存在しなければ `None`。
    pub fn get_property_value(&self, key: &PropertyKey) -> Option<&ClipBindingValue> {
        self.properties.get(key)
    }

    /// プロパティパス（`PropertyPath`）を辿って値を取得します。
    ///
    /// グループ型プロパティのメンバー値にアクセスする際に使用します。
    /// パスを辿れない場合（グループでない、メンバーが不在など）は `None` を返します。
    pub fn get_property_path(&self, path: &PropertyPath) -> Option<&FieldValue> {
        get_property_path_from_map(&self.properties, path)
    }

    /// プロパティパスを辿って値を設定します。
    ///
    /// ルートがグループ型の場合、メンバー単位で値を更新します。
    /// パスが不正な場合はエラーを返します。
    pub fn set_property_path(
        &mut self,
        path: &PropertyPath,
        value: ClipBindingValue,
    ) -> anyhow::Result<()> {
        if path.members().is_empty() {
            return self.set_property_value(path.root().clone(), value);
        }
        let target = self
            .properties
            .get_mut(path.root())
            .ok_or_else(|| anyhow::anyhow!("property group `{}` not found", path.root()))?;
        let ClipBindingValue::Static(target_value) = target;
        let mut current = target_value;
        for member in path.members() {
            let FieldValue::Map(values) = current else {
                anyhow::bail!("property path `{path}` traverses a non-group");
            };
            current = values
                .get_mut(member)
                .ok_or_else(|| anyhow::anyhow!("property path `{path}` not found"))?;
        }
        let ClipBindingValue::Static(value) = value;
        *current = value;
        Ok(())
    }

    /// 指定キーのプロパティ値を設定（上書き）します。
    pub fn set_property_value(
        &mut self,
        key: PropertyKey,
        value: ClipBindingValue,
    ) -> anyhow::Result<()> {
        self.properties.insert(key, value);
        Ok(())
    }
}

/// クリップ種別の定義です。
///
/// プラグインが `clips/` ディレクトリ下の TOML で宣言し、
/// `ClipKindRegistry` に登録されます。
///
/// `components` はこの種別のクリップを構成するコンポーネントの列で、
/// クリップの初期構成を担保します。コンポーネントは独立した部品として
/// 定義され、複数のクリップ種別から再利用できます。
#[derive(Debug, Clone)]
pub struct ClipKind {
    pub render_script: String,
    pub tags: HashSet<String>,
    /// このクリップ種別を構成するコンポーネント（宣言順 = 実行順）。
    pub components: Vec<NamespacedID>,
}

/// クリッププロパティの値です。
///
/// 現在は `Static(FieldValue)` のみですが、将来的にキーフレーム
/// （`Vec<(TimelineTick, FieldValue)>` 等）を追加予定です。
#[derive(
    Archive, rkyv::Deserialize, rkyv::Serialize, serde::Serialize, serde::Deserialize, Debug, Clone,
)]
#[archive_attr(derive(CheckBytes))]
pub enum ClipBindingValue {
    Static(FieldValue),
    // 将来的に Keyframes(Vec<(TimelineTick, FieldValue)>) 等を追加予定
}

impl ClipBindingValue {
    /// `Static` の内部 `FieldValue` への参照を返します。
    ///
    /// 将来キーフレームを追加したら、ここもそれに応じて変更する予定です。
    pub fn in_frame_of(&self) -> Option<&FieldValue> {
        match self {
            ClipBindingValue::Static(v) => Some(v),
        }
    }

    /// スキーマ一覧からデフォルト値のプロパティマップを構築します。
    pub fn default_properties(
        schema: &[PropertySchema],
    ) -> HashMap<PropertyKey, ClipBindingValue> {
        schema
            .iter()
            .map(|field| {
                (
                    field.key.clone(),
                    ClipBindingValue::Static(field.default.clone()),
                )
            })
            .collect()
    }
}

/// プロパティマップから `PropertyPath` を辿って値を取り出します。
///
/// `Clip::get_property_path` と同じ辿り方を、まだ `Clip` になっていない
/// プロパティマップに対して行います。
pub fn get_property_path_from_map<'a>(
    properties: &'a HashMap<PropertyKey, ClipBindingValue>,
    path: &PropertyPath,
) -> Option<&'a FieldValue> {
    let mut value = properties.get(path.root())?.in_frame_of()?;
    for member in path.members() {
        let FieldValue::Map(values) = value else {
            return None;
        };
        value = values.get(member)?;
    }
    Some(value)
}

/// プロパティマップへ `PropertyPath` を辿って値を設定します。
///
/// `Clip::set_property_path` と同じ辿り方を、まだ `Clip` になっていない
/// プロパティマップに対して行います。パスが不正な場合はエラーを返します。
pub fn set_property_path_in_map(
    properties: &mut HashMap<PropertyKey, ClipBindingValue>,
    path: &PropertyPath,
    value: ClipBindingValue,
) -> anyhow::Result<()> {
    if path.members().is_empty() {
        properties.insert(path.root().clone(), value);
        return Ok(());
    }
    let target = properties
        .get_mut(path.root())
        .ok_or_else(|| anyhow::anyhow!("property group `{}` not found", path.root()))?;
    let ClipBindingValue::Static(target_value) = target;
    let mut current = target_value;
    for member in path.members() {
        let FieldValue::Map(values) = current else {
            anyhow::bail!("property path `{path}` traverses a non-group");
        };
        current = values
            .get_mut(member)
            .ok_or_else(|| anyhow::anyhow!("property path `{path}` not found"))?;
    }
    let ClipBindingValue::Static(value) = value;
    *current = value;
    Ok(())
}

/// タイムラインへの参照種別です。
///
/// `Mirror` は複数クリップが同じ `TimelineId` を共有し、
/// `Independent` はこのクリップ専用のプライベートタイムラインを指します。
#[derive(
    Archive,
    rkyv::Deserialize,
    rkyv::Serialize,
    serde::Serialize,
    serde::Deserialize,
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
)]
#[archive_attr(derive(CheckBytes))]
pub enum CompositionRef {
    Mirror(TimelineId),
    Independent(TimelineId),
}

impl CompositionRef {
    /// 参照先の `TimelineId` を返します。
    pub fn timeline_id(&self) -> TimelineId {
        match self {
            CompositionRef::Mirror(id) | CompositionRef::Independent(id) => *id,
        }
    }
}

/// スクリプトに渡すパラメータです。現在はプレースホルダーです。
///
/// Rhai 等のスクリプトエンジンと連携する際に型を差し替える想定です。
#[derive(
    Archive,
    rkyv::Deserialize,
    rkyv::Serialize,
    serde::Serialize,
    serde::Deserialize,
    Debug,
    Clone,
    Default,
)]
#[archive_attr(derive(CheckBytes))]
pub struct ScriptParams(pub BTreeMap<String, String>);

impl PartialEq for Clip {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
impl Eq for Clip {}

impl PartialEq for ArchivedClip {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
impl Eq for ArchivedClip {}

#[cfg(test)]
#[path = "../../tests/unit/project/clip_tests.rs"]
mod tests;
