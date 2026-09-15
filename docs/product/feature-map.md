# Esotereel 機能マップ

## 機能一覧

### 1. プロジェクト管理機能
- **NewProject**: 新しいプロジェクト作成
- **ProjectAll**: プロジェクトメタデータ取得

### 2. タイムライン管理機能
- **Timeline作成**: タイムラインの作成
- **Layer操作**: レイヤーの追加、削除、移動
- **Folder操作**: フォルダーの追加、削除、移動
- **Outline管理**: 階層構造の管理

### 3. クリップ操作機能
- **Clip追加**: クリップの追加
- **Clip移動**: クリップの移動（位置、持続時間、レイヤー間）
- **Clip削除**: クリップの削除
- **Clip配置**: 既存クリップの再配置

### 4. ビデオストリーミング機能
- **InitStream**: ビデオストリーム初期化
- **FetchStreamData**: ビデオデータ取得

### 5. レンダリング機能
- **オフスクリーンレンダリング**: GPUレンダリング
- **テクスチャ更新**: ビデオテクスチャの更新

### 6. プラグイン機能
- **Pluginロード**: プラグインの読み込み
- **ClipKind定義**: クリップ種類の定義
- **Toolbar拡張**: ツールバーの拡張
- **Script実行**: スクリプトの実行

### 7. 同期機能
- **ChangeSet管理**: 変更差分の追跡
- **Incremental Update**: 増分更新
- **Client View管理**: クライアント表示範囲管理

### 8. ネットワーク機能
- **Request/Response**: リクエスト/レスポンス処理
- **バイナリプロトコル**: rkyvシリアライゼーション

## 機能間の依存関係

### プロジェクト管理 → タイムライン管理
- NewProjectはTimeline作成を利用
- ProjectAllはTimelineメタデータを生成

### タイムライン管理 → クリップ操作
- Layer操作はClip追加の前提
- Folder操作はOutline構造に影響
- Outline管理はLayer/Folder操作に依存

### クリップ操作 → 同期機能
- Clip追加/移動/削除はChangeSetを更新
- Clip操作はChunkIndexを無効化

### ビデオストリーミング → レンダリング
- InitStreamはVideoStreamerを作成
- FetchStreamDataはStreamPlayerにデータを提供
- StreamPlayerはレンダリングでテクスチャ更新に使用

### プラグイン機能 → クリップ操作
- ClipKind定義はClip追加で使用
- PluginロードはClipKindの前提

### 同期機能 → ネットワーク機能
- ChangeSetはResponse生成に使用
- Incremental Updateはネットワーク効率化

### ネットワーク機能 → 全機能
- すべての操作はRequest/Response経由
- バイナリプロトコルはデータ転送に使用

## データ生成・変更・消費の関係

### データ生成
- **NewProject**: Project、Timeline（デフォルト4 Layer）を生成
- **Timeline作成**: Timeline、Layer、Outlineを生成
- **Layer操作**: Layer、Outlineを生成/変更
- **Folder操作**: LayerFolder、Outlineを生成/変更
- **Clip追加**: Clip、ChangeSetを生成
- **Clip移動**: Clip、ChangeSetを変更
- **Clip削除**: Clipを削除、ChangeSetを変更
- **InitStream**: VideoStreamer、ResourceIdを生成
- **Pluginロード**: Plugin、ClipKindを生成

### データ変更
- **Clip移動**: Clipのposition、duration、layer_idを変更
- **Layer操作**: Layerのname、enabledを変更
- **Folder操作**: LayerFolderのname、opacity、blend_modeを変更
- **Outline操作**: Outlineの階層構造を変更

### データ消費
- **レンダリング**: Timeline、Clip、StreamPlayerを消費
- **ProjectAll**: Project、Timelineメタデータを消費
- **FetchClipsInRange**: Timeline、Clipを消費
- **同期**: ChangeSetを消費

## イベント発生

### Clip操作イベント
- Clip追加: `clips_upserted` イベント
- Clip移動: `clips_upserted` イベント
- Clip削除: `clips_removed` イベント

### Layer操作イベント
- Layer追加: `layers_upserted` イベント
- Layer削除: `layers_removed` イベント

### Outline操作イベント
- Folder追加: `outline_folders_upserted` イベント
- Folder削除: `outline_folders_removed` イベント
- 子ノード変更: `outline_children_changed` イベント

### ネットワークイベント
- Request受信: 対応するHandler呼び出し
- Response送信: GUI更新トリガー
- Dirty通知: 増分同期トリガー

## 機能グラフ

```
NewProject → Timeline作成 → Layer操作 → Clip操作 → ChangeSet → Response → GUI
                      ↓                ↓
                    Outline          ChunkIndex
                      ↓
                    Folder操作

InitStream → VideoStreamer → FetchStreamData → StreamPlayer → レンダリング

Pluginロード → ClipKind → Clip追加

ProjectAll → TimelineMeta → Response → GUI

FetchClipsInRange → Timeline.query_range → ChunkIndex → Response → GUI
```

## クロスカット機能

### Timeline ↔ Clip
- TimelineはClipを管理
- ClipはTimelineに属する
- Clip操作はTimelineのChangeSetを更新

### Timeline ↔ Outline
- TimelineはOutlineを管理
- OutlineはTimelineの階層構造を表現
- Layer/Folder操作は両方を更新

### Clip ↔ Plugin
- ClipはPluginのClipKindを参照
- PluginはClipの振る舞いを定義

### VideoStreamer ↔ StreamPlayer
- VideoStreamerはサーバー側でデコード
- StreamPlayerはクライアント側で再生
- FetchStreamDataでデータ転送

### ChangeSet ↔ Network
- ChangeSetは変更を追跡
- NetworkはChangeSetをResponseに変換
- Incremental Updateで効率化

## 未実装機能との依存

### アンドゥ/リドゥ
- CommandHistory構造は存在
- handle_command_actionは実装済み
- **Unknown**: GUI側のアンドゥ/リドゥ UI
- **Unknown**: 履歴スタックの管理
- **Unknown**: 再実行ロジック

### プロジェクト保存/読み込み
- Project構造はシリアライズ可能
- **Unknown**: ファイルフォーマット
- **Unknown**: 保存/読み込み処理
- **Unknown**: メディアファイルのパス管理

### エクスポート
- レンダリング機能は存在
- **Unknown**: エンコーダー統合
- **Unknown**: エクスポート設定
- **Unknown**: フォーマット選択

## Open Questions

1. **アンドゥ/リドゥ**: CommandHistory構造はあるが、GUI側の実装と履歴スタック管理が不明
2. **プロジェクト保存**: ファイルフォーマットと保存/読み込み処理が未実装
3. **エクスポート**: レンダリング結果のエンコードとファイル出力が未実装
4. **メディア管理**: メディアファイルのパス管理とリロケーションが不明
5. **Script連携**: Scriptの評価タイミングとTimeline生成の詳細が不明
6. **Nested Timeline**: Composite/Area2D/Area3Dのレンダリング実装が不明
7. **Audio**: オーディオ処理の実装有無が不明
