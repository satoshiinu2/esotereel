# プロパティのドロップターゲット設計メモ

## 概要

タイムラインへの動画ファイルドラッグドロップ時に、ドロップされたファイルパスをどのプロパティメンバーに書き込むかを、`ComponentCapability::Droppable` によって解決する設計。プロパティの**識別**は `member_capabilities` マップ内の `Droppable` 能力で行い、実際のプロパティ**書き込み**は `guihlp` 側で `AddClip` コマンドのプロパティ初期化時に行う。

## 背景と問題

現在の `guihlp/src/commands.rs` `req_cmd_add_clip_with_source` は、`std:video:media` / `"source"` をハードコードしている。

```rust
let media_key = NamespacedID::parse("std:video:media")?;
media_map.insert("source".to_string(), FieldValue::String(source_path.to_string()));
```

これは既存設計 (`docs/architecture/clip-properties.md` 行9) に違反：

> プロパティID（`std:video:media` など）をコード中にハードコードしてはならない。

また `ComponentCapability::MediaSource` をドロップ書き込み先に流用するのは不適切。`MediaSource` は「レンダラーが動画ソースを**読み取る**」ための能力であり、「GUI からドロップされたファイルパスを受け取ってプロパティを**書き込む**」責務とは別物。能力に違う仕事をさせるべきではない。

## 方針：Droppable 能力による識別

### 1. ドロップターゲットの識別（スキーマ側）

`ComponentCapability::Droppable` を `member_capabilities` マップの値として使用し、どのプロパティメンバーがドラッグドロップ書き込み宛先かを宣言する。

`PropertySchema` の `member_capabilities` フィールド（既存）：

```rust
pub struct PropertySchema {
    // ...既存フィールド...
    #[serde(default)]
    pub member_capabilities: BTreeMap<String, ComponentCapability>,
}
```

`Droppable` は `ComponentCapability` enum のバリアントとして定義済み（`as_str()` → `"droppable"`、`from_str()` → `"droppable"`、`standard_fn()` → `None`、`hook_timing()` → `ComponentHook::Evaluate`）。

TOML 宣言例：

```toml
[[components]]
id = "video"
properties = [
  { key = "video:media", label = "Media",
    kind = { type = "group", reference = "video:media" },
    fields = [
      { key = "source", label = "Source", kind = { type = "string" }, default = "" },
      { key = "source_offset", label = "Source Offset", kind = { type = "string" }, default = "0" },
    ],
    capability = "frame-provider",
    member_capabilities = { source = "droppable", source_offset = "media-offset-provider" },
    default = { source = "", source_offset = "0" } },
]
```

`member_capabilities` の `source` は `"droppable"`（GUI からのドロップ書き込み宛先マーカー）。`source_offset` は `"media-offset-provider"`（レンダラー用読み取り能力）。これにより「ドロップ書き込み宛先」と「レンダラー読み取り能力」が能力レベルで分離される。

### 2. プロパティ書き込み（guihlp 側）

識別されたドロップターゲットメンバーへのファイルパス書き込みは、`guihlp/src/commands.rs` の `req_cmd_add_clip_with_source` で `AddClip` コマンドのプロパティ初期化時に行う。

### コマンド側の解決ロジック

```rust
// member_capabilities 内の ComponentCapability::Droppable なメンバーを探す
let (prop_key, member_key) = property_schema
    .iter()
    .find_map(|prop| {
        prop.member_capabilities.iter()
            .find(|(_, cap)| **cap == ComponentCapability::Droppable)
            .map(|(member, _)| (prop.key.clone(), member.clone()))
    })
    .ok_or_else(|| anyhow::anyhow!("no drop-target property found"))?;
```

見つかった `(prop_key, member_key)` に対して、`source_path` を `FieldValue::String` としてプロパティマップに挿入する。

## Map vs Group について

`video:media` は `known_keys = ["source", "source_offset"]` で固定キーであり、実行時に動的にキーが増えることはない。Map より **Group** の方が自然。

| 比較項目 | Map | Group |
|---|---|---|
| キーの固定性 | 動的追加可能 | 固定（スキーマで定義） |
| 値の型 | すべて同じ `value_kind` | メンバーごとに異なる型を指定可能 |
| スキーマ厳格さ | `known_keys` で制限（緩やか） | フィールド定義で厳格 |
| GUI 表示 | メンバーを動的に列挙 | フィールド定義から静的に列挙 |
| スキーマ上の入れ子 | フラット | フィールド定義で入れ子構造を表現可能 |

Group の方が「スキーマ上は入れ子、ランタイムはフラット」という設計方針と整合する。`video:media` を Map から Group に変更する。

※ `PropertySchema::group()` はランタイムデフォルトを `FieldValue::Map` で構築するため、ランタイム値構造は Map と互換。

## ランタイムのフラット化と入れ子

スキーマ上は入れ子（グループ/マップ）で定義できるが、ランタイムのプロパティストレージはフラットに保つ。

- スキーマ: `std:video/media`（Group）→ `source`, `source_offset`
- ランタイム: フラットなキー値ストア。パスは `PropertyPath` (`std:video/media/source`) で指定
- マングリング: `PropertyPath` のセグメントを `/` で連結した文字列を内部キーとして使用、または `NamespacedID` の階層をそのまま保持

内部マングリングの仕様をここで決めておくことが重要。通信時のキー型は `NamespacedID` から `PropertyPath` に移行予定（既存設計 doc 行112-113 参照）。

## 実装ステップ

1. **`lib/src/plugin/property/mod.rs`** — `ComponentCapability::Droppable` バリアントを追加、`as_str()` / `from_str()` で `"droppable"` を処理、`PropertySchema` から `drop_target_members` フィールドを削除
2. **`lib/src/plugin/property/parse.rs`** — `PropertySchemaRaw` から `drop_target_members` フィールドを削除
3. **`plugins/std/components/video.toml`** — `video:media` を Map から Group に変更、`member_capabilities = { source = "droppable", source_offset = "media-offset-provider" }` を設定、`drop_target_members` を削除
4. **`guihlp/src/commands.rs`** — `req_cmd_add_clip_with_source` をハードコードから `member_capabilities` 内の `ComponentCapability::Droppable` ベースの逆引きに変更
5. **全テスト構築サイト** — `drop_target_members: BTreeMap::new(),` を削除
6. **ビルド確認** — `cargo build` でコンパイル通るか確認
7. **GUI ドラッグドロップ動作確認** — 既に実装済みの `drop.cpp` / `TimelineCanvasWidget.cpp` を動かす

## 関連ファイル

- `guihlp/src/commands.rs` - `req_cmd_add_clip_with_source` のドロップターゲット解決部分
- `guihlp/include/esotereel_gui_helper.h` - cbindgen 生成ヘッダ
- `lib/src/plugin/property/mod.rs` - `ComponentCapability` enum、`PropertySchema` 定義
- `lib/src/plugin/property/parse.rs` - `PropertySchemaRaw` 定義、`FieldTypeKind::from_toml_value`
- `plugins/std/components/video.toml` - `video:media` のスキーマ定義
- `docs/architecture/clip-properties.md` - 既存のプロパティ設計
