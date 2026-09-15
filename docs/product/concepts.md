# Esotereel ドメイン概念

## 主要概念

### Project
**Definition**: 複数のTimelineを管理する最上位コンテナ

**Responsibility**:
- Timelineの作成、管理、削除
- ID生成（IdGenerator）の管理
- Timeline間の変更伝播（propagate_nested_dirty）
- メタデータ提供（timelines_meta）

**Owned Data**:
- `timelines: BTreeMap<TimelineId, Timeline>`
- `ids: IdGenerator`

**References**: なし（最上位コンテナ）

**Created By**: NewProjectリクエスト、Project復元

**Modified By**: Timeline追加/削除、Clip操作による変更伝播

**Used By**: Coreサーバー、レンダリングコンテキスト

**Lifecycle**: NewProjectで作成、サーバー起動中維持

---

### Timeline
**Definition**: 時間軸を表すコンテナ。Layer、Clip、Outlineを管理

**Responsibility**:
- Layerの追加、削除、移動
- Clipの追加、削除、移動、配置
- Folderの階層構造管理（LayerOutline）
- 範囲検索（query_range、ChunkIndex）
- 変更追跡（ChangeSet）

**Owned Data**:
- `id: u64`
- `tps: f64` (ticks per second)
- `layers: HashMap<LayerId, Layer>`
- `clips: HashMap<ClipId, Clip>`
- `outline: LayerOutline`
- `chunk_index: RwLock<Option<ChunkIndex>>` (キャッシュ)
- `changes: ChangeSet` (未同期差分)

**References**: なし（独立したエンティティ）

**Created By**: Project::insert_timeline、Timeline::deep_clone

**Modified By**: Layer操作、Clip操作、Folder操作

**Used By**: Project、Clip、レンダリング

**Lifecycle**: Project作成時に自動生成、独立コピー可能

---

### Layer
**Definition**: クリップを保持するコンテナ。Video/Audio/Effect等の区別はない

**Responsibility**:
- 位置ベースのClip参照管理（position → ClipId）
- Clipの位置検索（get_clip_id_at）

**Owned Data**:
- `id: LayerId`
- `name: String`
- `enabled: bool`
- `clips: BTreeMap<i64, ClipId>` (位置→ClipId)

**References**: Clip（IDによる参照）

**Created By**: Timeline::insert_layer

**Modified By**: Clipの追加/削除

**Used By**: Timeline、Outline

**Lifecycle**: Timeline作成時にデフォルト4つ生成、追加可能

---

### Clip
**Definition**: タイムライン上の個別のメディア要素

**Responsibility**:
- 位置、持続時間の管理
- 種類（kind_id）、プロパティ、変換の保持

**Owned Data**:
- `id: ClipId`
- `position: TimelineTick`
- `duration: TimelineTick`
- `kind_id: NamespacedID`
- `properties: BTreeMap<String, PropertyValue>`
- `translates: ClipTranslates`

**References**: なし（データは自己完結）

**Created By**: Timeline::new_clip_in

**Modified By**: ClipsMoveコマンド

**Used By**: Layer、Timeline、レンダリング

**Lifecycle**: ユーザー操作で追加、削除、移動

---

### ClipData
**Definition**: Clipの実データ（種類ごとの異なる構造）

**Responsibility**:
- メディア種類ごとのデータ保持
- ネストされたTimeline参照

**Owned Data**: 種類ごとに異なる
- `Dummy`: データなし
- `Video`: path, media_offset
- `Audio`: path, media_offset
- `Composite`: source (CompositionRef)
- `Area2D`: source (CompositionRef)
- `Area3D`: source (CompositionRef)
- `Script`: script_id, params, generated (Option<TimelineId>)

**References**: Timeline（CompositionRef経由）

**Created By**: クリップ作成時

**Modified By**: スクリプト評価（generatedの更新）

**Used By**: Clip、レンダリング

**Lifecycle**: Clipと同期

---

### LayerOutline
**Definition**: レイヤー/フォルダーの階層構造と表示順序

**Responsibility**:
- 階層構造の管理（roots、folders）
- 実行順序のイテレーション（iter_execution_order）
- ノードの移動、削除

**Owned Data**:
- `roots: Vec<OutlineNode>`
- `folders: HashMap<LayerFolderId, LayerFolder>`

**References**: Layer、LayerFolder（IDによる参照）

**Created By**: Timeline作成時

**Modified By**: Layer/Folderの追加、削除、移動

**Used By**: Timeline、GUI

**Lifecycle**: Timelineと同期

---

### LayerFolder
**Definition**: レイヤーをグループ化するフォルダー

**Responsibility**:
- 子ノードの管理
- ブレンドモード、不透明度の管理

**Owned Data**:
- `name: String`
- `children: Vec<OutlineNode>`
- `opacity: f32`
- `blend_mode: BlendMode`

**References**: OutlineNode（Layer、Folder）

**Created By**: Timeline::insert_folder

**Modified By**: Folder操作

**Used By**: LayerOutline

**Lifecycle**: ユーザー操作で追加、削除

---

### ChangeSet
**Definition**: 未同期の変更差分

**Responsibility**:
- 変更の追跡（upserted、removed）
- ネットワーク同期用の差分提供

**Owned Data**:
- `clips_upserted: HashSet<ClipId>`
- `clips_removed: HashMap<ClipId, RemovedClipInfo>`
- `layers_upserted: HashSet<LayerId>`
- `layers_removed: HashSet<LayerId>`
- `outline_folders_upserted: HashSet<LayerFolderId>`
- `outline_children_changed: HashSet<Option<LayerFolderId>>`
- `outline_folders_removed: HashSet<LayerFolderId>`

**References**: 各エンティティのID

**Created By**: Timeline操作時に自動生成

**Modified By**: Timelineの各操作メソッド

**Used By**: Timeline、ネットワーク同期

**Lifecycle**: drain_changesで消費、次回操作で再生成

---

### CompositionRef
**Definition**: クリップからTimelineへの参照

**Responsibility**:
- ネストされたTimelineの参照種類を区別

**Owned Data**: なし（enum variant）

**References**: TimelineId

**Created By**: Clip作成時

**Modified By**: make_independent時

**Used By**: ClipData、レンダリング

**Lifecycle**: Clipと同期

---

### CommandRequest
**Definition**: プロジェクトに対する操作リクエスト

**Responsibility**:
- ユーザー操作の表現
- アンドゥ/リドゥ用の履歴生成

**Owned Data**: 種類ごとに異なる
- `ClipsMove`: Vec<ClipMoveCtx>
- `AddClip`: layer_id, position, duration, kind_id, properties, translates
- `AddLayer`: parent_folder_id, insert_index, name
- `AddFolder`: parent_folder_id, insert_index, name

**References**: LayerId、ClipId、NamespacedID

**Created By**: GUIからのユーザー操作

**Modified By**: なし

**Used By**: Core、CommandHistory

**Lifecycle**: リクエスト処理で消費

---

### CommandHistory
**Definition**: アンドゥ/リドゥ用のコマンド履歴

**Responsibility**:
- 実行されたコマンドの記録
- 再実行可能な状態の保持

**Owned Data**: CommandRequestとほぼ同じ構造

**References**: 同CommandRequest

**Created By**: command_to_history

**Modified By**: なし

**Used By**: Coreのアンドゥ/リドゥシステム

**Lifecycle**: 履歴管理

---

### NamespacedID
**Definition**: プラグイン内で一意なID

**Responsibility**:
- プラグイン間の名前衝突回避
- クリップ種類、プロパティの識別

**Owned Data**:
- `full: String` (plugin_id:local_id)
- `plugin_id: String`
- `local_id: String`

**References**: なし

**Created By**: プラグイン定義

**Modified By**: なし

**Used By**: Clip、PropertySchema、ToolbarButton

**Lifecycle**: プラグインロード時固定

---

### VideoStreamer
**Definition**: サーバー側のビデオデコードストリーマー

**Responsibility**:
- ビデオファイルのデコード
- パケットのバッチ取得
- ジェネレーション管理

**Owned Data**: FFmpeg内部状態

**References**: ビデオファイルパス

**Created By**: InitStreamリクエスト

**Modified By**: FetchStreamDataリクエスト

**Used By**: Coreサーバー

**Lifecycle**: InitStreamで作成、サーバー終了まで維持

---

### StreamPlayer
**Definition**: クライアント側のビデオ再生プレイヤー

**Responsibility**:
- デコード済みパケットの再生
- テクスチャ更新

**Owned Data**: デコードバッファ、テクスチャ

**References**: ResourceId

**Created By**: StreamMetadata受信時

**Modified By**: StreamData受信時

**Used By**: レンダリング

**Lifecycle**: プロジェクトオープン中維持

---

### ChunkIndex
**Definition**: 位置検索用の空間インデックス

**Responsibility**:
- 範囲クエリの高速化
- Clipの位置ベース検索

**Owned Data**: チャンクベースのインデックス構造

**References**: LayerId、ClipId、position

**Created By**: Timeline::query_range（遅延構築）

**Modified By**: Clip操作時に無効化、再構築

**Used By**: Timeline

**Lifecycle**: キャッシュ、無効化時に再構築

---

### Plugin
**Definition**: 機能拡張モジュール

**Responsibility**:
- ClipKindの定義
- PropertySchemaの提供
- ToolbarButtonの提供
- Scriptの提供

**Owned Data**:
- `manifest: PluginManifest`
- `setting_schemas: Vec<PropertySchema>`
- `clip_kinds: HashMap<NamespacedID, ClipKind>`
- `toolbar_buttons: Vec<ToolbarButtonSpec>`
- `script: Option<CompiledScript>`
- `dir: PathBuf`

**References**: なし

**Created By**: PluginLoader

**Modified By**: reload_plugin_by_id

**Used By**: PluginLoader、Clip作成

**Lifecycle**: アプリ起動時にロード、ホットリロード可能

---

### ClipKind
**Definition**: プラグイン定義のクリップ種類

**Responsibility**:
- クリップの振る舞い定義
- プロパティスキーマの提供

**Owned Data**:
- `func_name: String`
- `property_schema: Vec<PropertySchema>`

**References**: PropertySchema

**Created By**: プラグインのclips/*.toml

**Modified By**: プラグインリロード

**Used By**: Clip作成

**Lifecycle**: プラグインと同期
