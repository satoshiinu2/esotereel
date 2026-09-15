# UIとCoreの境界

## 境界の概要

Esotereelでは、GUI（Qt/C++）とCore（Rust）がTCP/IPネットワークを介して通信する。すべての操作はRequest/Responseパターンで行われ、各層の責務を明確に分離している。

## アーキテクチャレイヤー

```
┌─────────────────────────────────────────────────────────────┐
│                        GUI Layer                             │
│  (Qt/C++ - ユーザーインターフェースとプレゼンテーション)       │
└─────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────┐
│                     Bridge Layer                             │
│  (Rust FFI - C++ ↔ Rust インターフェース)                     │
└─────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────┐
│                   Network Layer                              │
│  (TCP/IP - カスタムバイナリプロトコル)                         │
└─────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────┐
│                      Core Layer                              │
│  (Rust - ビジネスロジックと状態管理)                          │
└─────────────────────────────────────────────────────────────┘
```

## 各操作の境界詳細

### NewProject

#### GUI State
- **Before**: プロジェクト未ロード状態
- **User Action**: 「新規プロジェクト」メニュー選択
- **GUI Processing**: メニューイベント処理

#### Request
- **C++ FFI**: `server_network_start()` または同等のFFI呼び出し
- **Rust FFI**: `Request::NewProject` シリアライズ
- **Network**: TCP送信

#### Core Processing
- **Request Handler**: `on_request_receive(ArchivedRequest::NewProject)`
- **Domain Logic**:
  - `Project::new()`
  - `Project::insert_timeline(60.0)`
  - `Project::timelines_meta()`
- **State Mutation**:
  - `ServerState.project` の更新
  - `ServerState.network.client_views` の初期化

#### Response
- **Response Type**: `Response::ProjectMeta { timelines }`
- **Network**: TCP送信

#### GUI Update
- **Response Handler**: `on_responce_recveve(Response::ProjectMeta)`
- **C++ Processing**: `Timeline::from_meta()` で骨格構築
- **GUI State Update**: タイムラインUI表示

---

### AddClip

#### GUI State
- **Before**: プロジェクトロード済み、レイヤー選択済み
- **User Action**: メディアファイルドロップ
- **GUI Processing**:
  - ファイルパス取得
  - 位置計算（ドロップ位置）
  - パラメータ構築（layer_id, position, duration, kind_id, properties, translates）

#### Request
- **C++ FFI**: `send_command()` または同等のFFI呼び出し
- **Rust FFI**: `Request::Command { command: CommandRequest::AddClip { ... }, timeline_id }` シリアライズ
- **Network**: TCP送信

#### Core Processing
- **Request Handler**: `on_request_receive(ArchivedRequest::Command)`
- **Domain Logic**:
  - `command_to_history()` - CommandRequest → CommandHistory
  - `handle_command_action()` - コマンド実行
  - `clip_add_core()` - クリップ追加ロジック
  - `Timeline::new_clip_in()` - タイムラインへの追加
- **State Mutation**:
  - `Timeline.clips` の更新
  - `Layer.clips` の更新
  - `Timeline.changes` の更新
  - `Timeline.chunk_index` の無効化

#### Response
- **Response Type**: `Response::UpdateClip { timeline_id, clips }`
- **Network**: TCP送信（Dirty通知経由）

#### GUI Update
- **Response Handler**: `on_responce_recveve(Response::UpdateClip)`
- **C++ Processing**: `Timeline::upsert_clip_from_network()`
- **GUI State Update**: タイムラインUI更新

---

### ClipsMove

#### GUI State
- **Before**: クリップ選択済み
- **User Action**: ドラッグ操作
- **GUI Processing**:
  - ドラッグ開始検知
  - リアルタイム位置計算
  - ドロップ位置決定
  - パラメータ構築（Vec<ClipMoveCtx>）

#### Request
- **C++ FFI**: `send_command()` または同等のFFI呼び出し
- **Rust FFI**: `Request::Command { command: CommandRequest::ClipsMove { ... }, timeline_id }` シリアライズ
- **Network**: TCP送信

#### Core Processing
- **Request Handler**: `on_request_receive(ArchivedRequest::Command)`
- **Domain Logic**:
  - `command_to_history()` - 現在状態の記録
  - `handle_command_action()` - コマンド実行
  - `clip_move_mul_core()` - 複数クリップ移動
  - `Timeline::place_clip()` - クリップ再配置
- **State Mutation**:
  - `Timeline.clips` の更新
  - `Layer.clips` の更新（複数レイヤー）
  - `Timeline.changes` の更新
  - `Timeline.chunk_index` の無効化

#### Response
- **Response Type**: `Response::UpdateClip { timeline_id, clips }`
- **Network**: TCP送信（Dirty通知経由）

#### GUI Update
- **Response Handler**: `on_responce_recveve(Response::UpdateClip)`
- **C++ Processing**: `Timeline::upsert_clip_from_network()`
- **GUI State Update**: タイムラインUI更新

---

### AddLayer/AddFolder

#### GUI State
- **Before**: プロジェクトロード済み
- **User Action**: 「レイヤー追加」または「フォルダー追加」選択
- **GUI Processing**:
  - 名前入力ダイアログ
  - 親フォルダー選択
  - 挿入位置指定

#### Request
- **C++ FFI**: `send_command()` または同等のFFI呼び出し
- **Rust FFI**: `Request::Command { command: CommandRequest::AddLayer { ... } or AddFolder { ... }, timeline_id }` シリアライズ
- **Network**: TCP送信

#### Core Processing
- **Request Handler**: `on_request_receive(ArchivedRequest::Command)`
- **Domain Logic**:
  - `command_to_history()` - CommandRequest → CommandHistory
  - `handle_command_action()` - コマンド実行
  - `Project::insert_layer_in_timeline()` or `Project::insert_folder_in_timeline()`
  - `Timeline::insert_layer()` or `Timeline::insert_folder()`
- **State Mutation**:
  - `Timeline.layers` の更新（AddLayerのみ）
  - `Timeline.outline` の更新
  - `Timeline.changes` の更新

#### Response
- **Response Type**:
  - AddLayer: `Response::UpdateLayer { timeline_id, layers }` + `Response::UpdateOutline`
  - AddFolder: `Response::UpdateOutline { timeline_id, folders, children }`
- **Network**: TCP送信（Dirty通知経由）

#### GUI Update
- **Response Handler**: `on_responce_recveve(Response::UpdateLayer or UpdateOutline)`
- **C++ Processing**: `Timeline::apply_layer_meta()` or `Timeline::apply_outline_folder_meta()`
- **GUI State Update**: レイヤー/フォルダーUI更新

---

### InitStream

#### GUI State
- **Before**: プロジェクトロード済み
- **User Action**: ファイル選択ダイアログ
- **GUI Processing**:
  - ファイルパス取得
  - ファイル存在確認

#### Request
- **C++ FFI**: `init_stream()` または同等のFFI呼び出し
- **Rust FFI**: `Request::InitStream { path }` シリアライズ
- **Network**: TCP送信

#### Core Processing
- **Request Handler**: `on_request_receive(ArchivedRequest::InitStream)`
- **Domain Logic**:
  - `VideoStreamer::new(path)` - FFmpeg初期化
  - `state.get_or_create_resource_id(path)` - ResourceId生成
  - `VideoStreamer::get_init_packet()` - メタデータ取得
- **State Mutation**:
  - `ServerState.streams` の更新
  - `ServerState.path_to_stream` の更新

#### Response
- **Response Type**: `Response::StreamMetadata { path, resource_id, codec_id, width, height, time_base, extradata }`
- **Network**: TCP送信

#### GUI Update
- **Response Handler**: `on_responce_recveve(Response::StreamMetadata)`
- **C++ Processing**: StreamPlayer作成
- **GUI State Update**: メディアライブラリUI更新

---

### FetchClipsInRange

#### GUI State
- **Before**: プロジェクトロード済み
- **User Action**: タイムラインスクロール
- **GUI Processing**:
  - 表示範囲計算
  - 範囲変更検知

#### Request
- **C++ FFI**: `fetch_clips_in_range()` または同等のFFI呼び出し
- **Rust FFI**: `Request::FetchClipsInRange { timeline_id, range }` シリアライズ
- **Network**: TCP送信

#### Core Processing
- **Request Handler**: `on_request_receive(ArchivedRequest::FetchClipsInRange)`
- **Domain Logic**:
  - `state.network.update_client_view()` - クライアント表示範囲記録
  - `Timeline::query_range()` - 範囲検索
  - ChunkIndex遅延構築（必要な場合）
- **State Mutation**:
  - `ServerState.network.client_views` の更新
  - ChunkIndex構築（キャッシュ）

#### Response
- **Response Type**: `Response::UpdateClip { timeline_id, clips }`
- **Network**: TCP送信

#### GUI Update
- **Response Handler**: `on_responce_recveve(Response::UpdateClip)`
- **C++ Processing**: `Timeline::merge_fetched_clips()`
- **GUI State Update**: タイムラインUI更新

---

## 責務の分離

### GUI Layerの責務
- ユーザー入力の受付
- UIの表示と更新
- ユーザー操作のパラメータ構築
- レスポンスのUI反映
- **しない**: ビジネスロジック、状態管理、データ永続化

### Bridge Layerの責務
- C++ ↔ Rust のデータ変換
- FFI関数の提供
- ネットワーククライアントの管理
- シリアライゼーション/デシリアライゼーション
- **しない**: ビジネスロジック、UIロジック

### Network Layerの責務
- TCP通信の管理
- プロトコルの実装
- クライアント接続管理
- メッセージルーティング
- **しない**: ビジネスロジック、データ構造

### Core Layerの責務
- ビジネスロジックの実行
- 状態管理（Project, Timeline, Clip等）
- コマンド実行
- 変更追跡
- ビデオデコード
- **しない**: UI、ネットワークプロトコル詳細

## データの所有権

### GUIが所有するデータ
- UI状態（選択、フォーカス、スクロール位置）
- 表示用のTimeline骨格（Clipなし）
- ユーザー設定

### Coreが所有するデータ
- Project実体
- Timeline実体（Clip含む）
- VideoStreamer
- ResourceIdマッピング
- CommandHistory

### 共有データ（同期）
- Timelineメタデータ（ProjectMeta）
- Clipデータ（UpdateClip経由）
- Layerデータ（UpdateLayer経由）
- Outlineデータ（UpdateOutline経由）

## 同戦略

### 変更通知フロー
1. Coreで状態変更
2. ChangeSet更新
3. Dirty通知（`notify_dirty()`）
4. 変更同期タスク起動
5. ChangeSetをResponseに変換
6. 対象クライアントに送信
7. GUIで状態反映

### 増分更新
- ChangeSetに変更があったエンティティのみを送信
- クライアントの表示範囲を考慮して最適化
- `clients_watching_in()` で対象クライアントをフィルタリング

### 競合処理
- **Unknown**: 複数クライアントの編集競合処理
- **Unknown**: 楽観的ロック/悲観的ロックの使用
- **Unknown**: 競合検出と解決メカニズム

## Open Questions

1. **GUI側のオフライン操作**: ネットワーク切断時のGUI操作の扱いは？
2. **リアルタイムプレビュー**: プレビュー用の専用通信チャネルはあるか？
3. **バッチ操作**: 複数コマンドのバッチ送信は可能か？
4. **エラーハンドリング**: GUI側でのエラー表示とリカバリー戦略は？
5. **状態同期**: GUIとCoreの状態不一致の検出と修正は？
6. **パフォーマンス**: 大量クリップ時の更新戦略は？
