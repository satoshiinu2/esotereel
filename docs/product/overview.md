# Esotereel プロダクト概要

## システムの目的

Esotereelは、クリエイター向けのハイブリッドアーキテクチャ（Qt/C++ GUI + Rust コア）を採用したビデオ編集ソフトウェアである。

## 実装されている機能

### 1. プロジェクト管理
- **NewProject**: 新しい空のプロジェクトを作成（デフォルトタイムライン付き）
- **ProjectAll**: プロジェクトメタデータの取得

### 2. タイムライン管理
- **Timeline作成**: FPS指定でタイムライン作成（デフォルト4レイヤー）
- **Layer管理**: レイヤーの追加、削除、移動
- **Folder管理**: フォルダーの追加、削除、移動（階層構造サポート）
- **Outline管理**: レイヤー/フォルダーの階層構造管理

### 3. クリップ管理
- **Clip追加**: レイヤーへのクリップ追加（重複チェックあり）
- **Clip移動**: 位置、持続時間、レイヤー間の移動
- **Clip削除**: クリップの削除
- **Clip配置**: 既存IDのクリップを再配置（undo/redo用）
- **範囲検索**: 指定範囲内のクリップ検索（ChunkIndex使用）

### 4. ビデオストリーミング
- **InitStream**: ビデオファイルからのストリーム初期化
- **FetchStreamData**: 指定時間範囲のビデオパケット取得
- **VideoStreamer**: サーバー側のビデオデコード
- **StreamPlayer**: クライアント側のビデオ再生

### 5. レンダリング
- **オフスクリーンレンダリング**: wgpuによるGPUアクセラレーション
- **ビデオテクスチャ更新**: ストリームデータからのテクスチャ更新
- **バッチレンダリング**: テクスチャごとのバッチ処理

### 6. プラグインシステム
- **PluginLoader**: プラグインの動的読み込み
- **ClipKind**: プラグイン定義のクリップ種類
- **ToolbarButton**: ツールバーボタンの拡張
- **Script**: Rhaiスクリプトエンジンによるスクリプト実行

### 7. 変更追跡と同期
- **ChangeSet**: 未同期の差分管理
- **Incremental Updates**: 増分更新によるネットワーク効率化
- **Client View**: クライアントの表示範囲管理

### 8. ネットワーク通信
- **TCP/IP**: カスタムバイナリプロトコル
- **rkyv**: ゼロコピーシリアライゼーション
- **Request/Response**: すべての操作はリクエスト/レスポンスパターン

### 9. デバッグ機能
- **DebugFetchProjectStruct**: プロジェクト構造のデバッグ用取得

## 実装されていない機能

- オーディオ編集
- エクスポート機能
- アンドゥ/リドゥ（コマンド履歴構造はあるが実装は不完全）
- 高度なビデオエフェクト
- トランジション
- プロジェクトの保存/読み込み（構造はあるが実装は不完全）

## アーキテクチャ

### コンポーネント
- **GUI (gui/)**: Qt6/C++ ユーザーインターフェース
- **Bridge (guihlp/)**: Rust FFIブリッジライブラリ
- **Core (core/)**: Rust コアサーバーアプリケーション
- **Shared Library (lib/)**: 共有Rustライブラリ

### 通信フロー
```
User Action → GUI → C++ FFI → Rust FFI → Network → Core → Domain Logic → State Change → Response → GUI Update
```

### データモデル
- **Project**: 複数のTimelineを管理
- **Timeline**: Layer、Clip、Outlineを管理
- **Layer**: クリップのコンテナ（位置→ClipIdマップ）
- **Clip**: 位置、持続時間、種類、プロパティ、変換
- **LayerOutline**: 階層構造（Folder、Layerの並び順）
- **ChangeSet**: 差分追跡（clips_upserted, clips_removed, etc.）

## 技術スタック

- **GUI**: Qt6 (C++20)
- **Core**: Rust (2024 edition)
- **Graphics**: wgpu (WebGPU)
- **Video**: FFmpeg (ffmpeg-next)
- **Serialization**: rkyv
- **Async**: tokio
- **Scripting**: Rhai
- **Build**: CMake + Cargo
