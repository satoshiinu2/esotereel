# Open Questions

このドキュメントは、Esotereelの現在の実装からでは判断できない仕様や、AIエージェントが実装時に判断が必要な箇所をまとめたものです。

2026-11-19に master `8f511c4` のコードと照合して見直した。コードを読めば答えが出る項目は「現状」へ移し、不明点には未実装・未決定のものだけを残してある。

## 1. アンドゥ/リドゥ機能

### 現状
- `HistoryStack`（`core/src/project/history.rs`）が undo/redo スタックを管理する。上限は100件で、push時は redo をクリアし、上限超過時は最古の履歴を落とす
- `command_to_history()`（`core/src/project/commands.rs`）でCommandRequestをCommandHistoryに変換する。実行は `core/src/requests.rs` の Command / Undo / Redo ハンドラが行い、再実行は `execute_command` / `execute_command_undo` が担う
- GUI側のUIは実装されている。専用メニューではなく、std プラグインのツールバーボタン（`plugins/std/toolbars.toml`、`plugins/std/buttons/undo.rhai` / `redo.rhai`）から `Request::Undo` / `Request::Redo` を送る形である
- 履歴は実行が成功した後にだけ積まれる（`execute_command_with_history`）。失敗したコマンドで履歴が汚染されることはない
- 履歴は `ServerState` のメモリ上（TimelineIdごとの `HistoryStack`）にのみあり、プロジェクト保存の対象ではない

### 不明点
- **部分適用時の巻き戻し**: 複合コマンドの途中失敗で先行分が Project に残る作り（トランザクション的な巻き戻しは無い）。巻き戻しを実装するかどうかが未決定

### 影響範囲
- `core/src/project/history.rs`
- `core/src/project/commands.rs`
- GUI側のアンドゥ/リドゥ実装

---

## 2. プロジェクト保存/読み込み

### 現状
- 部品（`Timeline`、`Clip`、`IdGenerator`）は serde 対応済みだが、`Project` 本体と `AssetRegistry` は serde 未対応で、Project 全体をシリアライズできない
- `lib/src/project/save.rs` は空（モジュール宣言のみ）で、master に保存/読み込みの実装は無い。GUI側の保存/読み込みUIも無い
- 実装は未マージの `feat/project-save-load` ブランチにのみある。同ブランチで決まっていること: フォーマットはバージョン付きJSON（v1、エンベロープ `{version, project}`）、未対応バージョンはエラーで拒否、メディアパスは絶対パスのまま相対化しない、圧縮はしない

### 不明点
- **ブランチ実装の採否**: `feat/project-save-load` の実装を採るかどうか（マージ判断）
- **保存対象**: Project全体か、メタデータのみか？（ブランチ実装は Project 全体）
- **メディアファイルのパス管理**: 絶対パスのままか、相対化するか？
- **外部ファイルの扱い**: メディアファイルのコピー/リンクは？
- **GUI配線**: 保存/読み込みUIの形は？

### 影響範囲
- `lib/src/project/save.rs`
- `lib/src/project/project.rs`
- GUI側の保存/読み込みUI

---

## 3. エクスポート機能

### 現状
- レンダリング機能（`render_frame_offscreen`）は実装されている
- wgpuによるオフスクリーンレンダリングが可能

### 不明点
- **エンコーダー統合**: FFmpegエンコーダーの統合方法は？
- **エクスポート設定**: 解像度、コーデック、ビットレート等の設定は？
- **フォーマット選択**: サポートするエクスポートフォーマットは？
- **プログレス表示**: エクスポート進捗の表示方法は？
- **品質設定**: 品質と速度のバランス調整は？
- **オーディオ**: オーディオのエクスポートは？

### 影響範囲
- 新規エクスポートモジュール
- `lib/src/render/` の拡張
- GUI側のエクスポートUI

---

## 4. メディア管理

### 現状
- `Project` はアセット台帳 `AssetRegistry`（`lib/src/project/asset.rs`）を持つ。`AssetEntry` は `AssetAvailability`（Available / Missing）と `AssetLocation`（External / Managed）を持ち、付け替え用に `relink`、解決に `resolve` がある
- アップロードの受け側は `core/src/assets.rs` の `UploadManager` が担う。台帳は持たず、ファイルの置き場所と転送進捗だけを管理し、完了したファイルを呼び出し側が `AssetRegistry` へ登録する
- 取り込み経路は3つある: (a) ファイルドロップ（`AddClipFromFile` → core が初期プロパティを作って通常の AddClip 化）、(b) アップロード（`BeginAssetUpload` → `AssetUploadChunk` → `FinishAssetUpload`）、(c) 参照登録（`RegisterAssetReference`）
- `InitStream` はクリップ追加時ではなく描画時の遅延生成である。描画時に必要なパスが `MediaFetchCache` に記録され、未ロードのパスについて `Request::InitStream` が生成される
- アップロード途中のアセットもストリーム再生できる。`InitStream` のパスが `UploadManager` の進行中セッションと一致する場合、`VideoStreamer::new_growing` で開き、入力（`lib/src/decode/growing.rs` の `GrowingFile`）は未到着バイトで EOF を返さず到着まで待つ。到着は先頭から連続した接頭辞として管理され、接頭辞の外は読まない。中断したセッションは待機中の読み側をエラーで起こす。制約として、moov が末尾にある MP4 は moov が届くまでデコードを開始できない
- デコード要求のキャッシュは `MediaFetchCache`、フレームの保持は `StreamPlayer` 側のバッファで、台帳とは別層である

### 不明点
- **リロケーション**: `relink` API はあるが、それを呼ぶ GUI/コマンド経路は未確認。プロジェクト移動時のパス修正をユーザーへどう提示するか？
- **欠落ファイル**: `Missing` という型表現はあるが、実ファイルが消えたときにエントリを `Missing` へ遷移させる経路は未確認。欠落時の確定した扱いは？
- **メモリ管理**: 大きなファイルのメモリ使用の上限方針は？
- **オーディオファイル**: オーディオファイルの取り扱いは？（§7参照、取り扱い自体が無い）

### 影響範囲
- `lib/src/decode/`
- `core/src/state.rs`
- GUI側のメディアライブラリ

---

## 5. Script連携

### 現状
- コンポーネントスクリプトは実行系に接続されている。`Scheduler`（`lib/src/plugin/clip/component.rs`）が能力（capability）ごとに標準関数をフックタイミング順に呼び、スクリプトの無いコンポーネントはスキップする
- props は [[requires]] 宣言でサンドボックス化されている（`lib/src/plugin/script/props.rs` の `ComponentProps::build`）。自プロパティは `:#name` 形式でのみ、依存は `[[requires]]` で宣言した分だけが解決でき、未宣言名へのアクセスはエラーになる
- Rhaiエンジンはホスト共有で、`CommonState.script_engine` として保持される。`ScriptStore` と `CompiledScript` が存在する
- スクリプトのエラーは、どのコンポーネントのどの能力関数が失敗したかを付けて伝播する。描画側では定義解決に失敗したクリップを警告ログを出してスキップする
- Rhaiエンジンのリソース上限（`max_operations` 等）は未設定で、デフォルトのままである
- `ScriptParams` は定義されているが、現行 Clip のプロパティや Rhai 実行には接続されていない（プレースホルダー）。`Clip` にスクリプト用フィールドは無く、代わりに `shader_stack` が追加されている

### 不明点
- **クリップ固有スクリプト**: Clip のプロパティを使うユーザー作成スクリプトを、どの API と評価モデルで支援するか？（現行はプラグイン同梱コンポーネントのスクリプトのみ）
- **Timeline生成**: 生成クリップや Timeline を導入する場合の生成・更新ライフサイクルは？（生成 API 自体が無い）
- **リソース上限**: Rhaiエンジンに上限を設定するか、するなら値と超過時の扱いは？
- **デバッグ**: スクリプトのデバッグ方法は？
- **パフォーマンス**: スクリプト評価のパフォーマンス影響は？（未計測）

### 影響範囲
- `lib/src/plugin/script/`
- `lib/src/project/clip.rs`
- レンダリングパイプライン

---

## 6. Nested Timeline

### 現状
- `CompositionRef` は Mirror / Independent の Timeline 参照を表す型として維持されているが、`Clip` 本体にこれを保持するフィールドは無い
- `Clip` との関連付けとネスト Timeline の描画パイプラインは未実装
- `Project::make_independent()` が実装されている。ただし呼び出し側で `Clip` の `CompositionRef` を差し替える前提で、その呼び出し側自体が無い
- `MAX_NESTED_DEPTH`（`lib/src/project/timeline.rs`、値32）は定義のみで、使用箇所は無く、深さ制限はどこにも適用されていない

### 不明点
- **レンダリング実装**: ネストされたTimelineのレンダリングは？
- **Mirror参照**: 複数クリップからのMirror参照の扱いは？
- **Independent化のトリガー**: いつIndependent化するか？
- **変更伝播**: ネストされたTimelineの変更伝播は？
- **パフォーマンス**: 深いネストのパフォーマンス影響は？

### 影響範囲
- `lib/src/project/clip.rs`
- `lib/src/project/project.rs`
- `lib/src/render/`

---

## 7. オーディオ処理

### 現状
- FFmpegはオーディオデコードをサポート

### 不明点
- **実装有無**: オーディオ処理は実装されているか？
- **波形表示**: オーディオ波形の表示は？
- **オーディオエフェクト**: オーディオエフェクトの適用は？
- **ミキシング**: 複数オーディオのミキシングは？
- **同期**: オーディオとビデオの同期は？
- **エクスポート**: オーディオのエクスポートは？

### 影響範囲
- 新規オーディオモジュール
- `lib/src/decode/` の拡張
- GUI側のオーディオUI

---

## 8. GUI側の実装詳細

### 現状
- C++ FFIラッパーが存在
- Qt6ベースのGUI
- ドラッグ&ドロップは詳細実装がある。タイムライン上のクリップドラッグ、範囲選択、リサイズ、ゴースト描画、外部ファイルのドロップ（`handleVideoFileDrop`）まで揃っている
- リアルタイムプレビューに専用通信チャネルは無い。プレビューは同一プロセス内の FFI 直接呼び出し（`render_frame_offscreen`）で描き、メディアの初期化/取得要求だけを通常の TCP `Request` でサーバーへ送る
- キーボードショートカットは実質未実装（`QShortcut` / `setShortcut` の使用なし、Undo/Redo のキー割り当ても無い）
- テーマ切替（ダークモード等）は無い。固定の stylesheet のみ
- アクセシビリティ対応コードは無い
- サーバーは同一マシン上の内蔵サーバーで、「ネットワーク切断」に相当するのは localhost 接続の切断である。その場合クライアントは受信ループを抜けて終了するだけで、再接続もオフライン継続操作も無い

### 不明点
- **切断時の扱い**: 内蔵サーバー前提で再接続は不要とするか、再接続を入れるか？

### 影響範囲
- `gui/src/`
- `guihlp/src/ffi/`

---

## 9. ネットワークと競合処理

### 現状
- TCP/IPベースの通信
- 複数クライアント接続をサポート
- クライアントごとの表示範囲を `client_views` で追跡する。削除関数 `remove_client_view(client_id)` は定義されているが、切断処理への接続は未接続と確認した（呼び出し箇所は0件で、切断時のクリーンアップは `clients` の削除と送信タスクの停止のみ）。切断したクライアントの表示範囲は残り続ける（REFACTORING_AUDIT の RF-009 参照）
- 競合処理は専用のロック戦略ではなく、次の組み合わせである: (a) 単一の権威サーバーが状態を Mutex で直列に処理する、(b) コマンドにステールネス検出がある（`base_position` / `base_duration` 等、クライアントが認識している現在の値と照合する）、(c) 拒否時は `Response::CommandRejected` を返し、クライアントに正しい状態で同期し直させる
- 通信バッファは送受信とも `mpsc::unbounded_channel` で、サイズ上限は無い
- 認証と暗号化は無い（素の TCP）。GUI用は localhost 接続だが、単体サーバーバイナリは `0.0.0.0:12345` で待ち受ける
- クライアント側に再接続処理は無い（§8参照）

### 不明点
- **同時編集の意味論**: 現行の「権威サーバー＋ステールネス拒否」を正式な仕様とするか、今後どうするか？
- **切断クリーンアップの接続**: `remove_client_view` を切断経路へ接続する（小さな実装判断。実施自体は RF-009 の推奨）
- **切断回復**: 再接続時の回復をどうするか？（§8の切断時方針と連動）

### 影響範囲
- `core/src/network.rs`
- `guihlp/src/network.rs`
- プロトコル設計

---

## 10. パフォーマンスとスケーラビリティ

### 現状
- ChunkIndexによる範囲検索の最適化
- 増分更新によるネットワーク効率化
- 計測の仕組み自体が無い。ベンチマーク（`benches/` や criterion 等）は存在せず、以下の項目はいずれも「不明」ではなく「未計測」である

### 不明点（いずれも未計測）
- **大量クリップ**: 1000+クリップ時のパフォーマンスは？
- **大規模プロジェクト**: プロジェクトサイズの上限は？
- **メモリ使用**: メモリ使用量の最適化戦略は？
- **GPUメモリ**: GPUメモリの管理は？
- **並列処理**: 並列レンダリングの実装は？（現行レンダーは逐次）
- **プロファイリング**: パフォーマンスプロファイリングツールは？（計測に踏み出すかどうか自体が判断事項）

### 影響範囲
- 全体のアーキテクチャ
- `lib/src/project/chunk_index.rs`
- `lib/src/render/`

---

## 11. エラーハンドリング

### 現状
- `EsotereelError` と `EsotereelResult` が定義されている（`lib/src/util/result.rs`）。エラー種には Asset 系（`AssetNotFound` / `AssetAlreadyExists` / `UploadFailed` / `FileDropFailed`）も含まれる
- ログ出力が実装されている。ログレベルは設定可能で、起動前設定と設定スキーマ（`core log.level` / `core log.filters`）があり、GUI の設定画面から扱える
- GUIエラー表示は実装されている。バックグラウンドスレッドの非同期エラーを `ClientState` に溜め、FFI の `client_state_take_error` で取得・クリアする。C++ 側は `ClientState::takeAsyncError()` を MainWindow が QTimer 1秒間隔で回収し、`qWarning()` とステータスバー表示を行う（モーダルは出さない）
- FFI境界では `FfiResult` / `FfiResultVoid` がエラーの原因チェーン込みで文字列化して返し、各 FFI 関数は `catch_unwind` でパニックもエラー化する。サーバーからの失敗通知は `Response::Failed` で届く
- リカバリーは部分的にある: ロック毒性の回復（`LockExt` / `MutexExt`）、コマンド拒否時の再同期（§9）、プラグイン読み込みの build-then-swap（失敗時に旧インデックス無傷）

### 不明点
- **エラーレポート**: エラーレポートの収集は？（仕組み自体が無い）
- **ユーザーフィードバック**: ステータスバー止まりの表示を、どこまでユーザーガイダンスに育てるか？

### 影響範囲
- 全体のエラーハンドリング
- GUI側のエラーUI
- ログシステム

---

## 12. プラグインシステム

### 現状
- `PluginLoader` が実装されている
- ClipKind、ToolbarButton、Scriptの定義が可能
- コンポーネント宣言（`components/*.toml` の `ComponentDefinition`: 能力、プロパティ、スクリプト、`[[requires]]` 依存宣言）が中核になっている。std プラグインは media / transform / video の3部品構成
- シェーダ宣言（`shaders/*.toml` ＋ WGSL の `ShaderDefinition`、読み込み時のコンパイル検証、Clip の `shader_stack` への適用、compute の重ねがけ）が lib に入っている
- ホットリロードの API は実装されている（`load_additional_plugin` / `reload_plugin_by_id` / `unload_plugin_by_id`）。再構築は build-then-swap で、失敗時に旧状態を壊さない
- プラグイン開発者向けドキュメントは `docs/plugins/`（overview / components / script-api / tutorial）にある
- マニフェストは `id` / `name` / `version` のみで、互換性検証は無い
- プラグイン間の依存関係を宣言する機構は無い。`[[requires]]` はプラグイン内部のコンポーネント間依存であり、プラグイン同士の依存ではない
- 配布は「std（同梱）／ユーザーのプラグインディレクトリへフォルダ配置」のみ
- mixin は master には無い。宣言の読み込み（フェーズ0）までが `feat/plugin-mixin` ブランチにのみあり、dispatch は未実装

### 不明点
- **依存関係**: プラグイン間の依存関係管理は？
- **互換性**: プラグインのバージョン互換性の検証は？
- **配布**: フォルダ配置以外の配布方法は？
- **セキュリティ**: props のサンドボックス化は入ったが、Rhai エンジン自体のリソース上限は無い（§5参照）。それ以外のサンドボックスは？
- **ホットリロード**: 仕組みはあるが、安全性の検証をどこまで厚くするか？

### 影響範囲
- `lib/src/plugin/`
- プラグイン開発ドキュメント

---

## 13. テスト

### 現状
- 単体テストは各クレートの `tests/unit/` に分離されている（`src` 側は `#[cfg(test)]` ＋ `#[path]` の宣言のみ）。件数は lib 116件 / core 7件 / guihlp 2件の計125件（master `8f511c4` 時点）
- うち5件は wgpu アダプタや実メディアを要するため `#[ignore]` で通常実行から外されている（通常実行は lib 111件 / core 7件 / guihlp 2件）。実メディア依存のテストは `ESOTEREEL_TEST_VIDEO` 環境変数で渡す運用
- CI で `cargo test --workspace --release` が実行されている
- 統合寄りの確認として `scripts/smoke-test.sh` ＋ `core/examples/smoke_client.rs` のサーバー疎通スモークがある。GUI を通す自動 E2E テストは無い（CI の起動スモークが最も近い）
- ベンチマークは無い（§10参照）

### 不明点
- **どの契約を固定するか**: 目標カバレッジのような数値目標は置かない（テストポリシー: 数やカバレッジではなく観測可能な振る舞い・契約を検証する）。未固定の契約は REFACTORING_AUDIT の RF-011（network 往復、dispatch の差分→送信対応、ビュー契約）に絞ってある
- **E2Eテスト**: エンドツーエンドテストの戦略は？

### 影響範囲
- 全体のテスト戦略
- CI/CDパイプライン

---

## 14. ローカライゼーション

### 現状
- `esotereel_ja.ts` と `esotereel_en.ts` が存在するが、どちらも空の TS ファイルで、翻訳運用は未開始である
- ビルドへの配線（`qt_add_translations`）のみがある。`QTranslator` の install は無く、`tr()` の使用もごく少数に限られる

### 不明点
- **翻訳範囲**: 翻訳対象の範囲は？
- **更新プロセス**: 翻訳の更新プロセスは？
- **複数言語**: 2言語以上のサポートは？
- **フォント**: 多言語フォントの対応は？
- **日付/時刻**: ロケールに応じたフォーマットは？

### 影響範囲
- GUI側のローカライゼーション
- 翻訳ファイル管理

---

## 15. ビルドとデプロイ

### 現状
- CMake + Cargoのハイブリッドビルド
- Linux, Windows, macOS対応
- インストーラーは3プラットフォームとも実体がある: Linux は `installer/linux/build_appimage.sh`（AppImage）、macOS は `installer/macos/build_dmg.sh`（DMG）、Windows は `installer/windows/build_installer.ps1` ＋ `installer/windows/installer.nsi`（NSIS）
- CI（`.github/workflows/ci.yml`）は単一ジョブ `build-and-test` の6構成マトリクス（ubuntu-22.04 / windows-latest / macos-latest / macos-15-intel / ubuntu-24.04-arm / windows-11-arm）で、Qt は 6.12.0 にピン留めされている。Rust テスト、各 OS のパッケージング、起動スモークまで含む
- 自動更新の実装は無い。コンテナ化（Dockerfile 等）も無い
- 依存関係（Qt/FFmpeg）は CI が都度取得し、インストーラーが同梱する形である

### 不明点
- **自動更新**: 自動更新機能の実装は？
- **配布**: 配布チャネルと戦略は？（成果物の生成までは CI で確認できるが、公開先の運用はリポジトリからは未確認）

### 影響範囲
- `CMakeLists.txt`
- `installer/`
- ビルドシステム

---

## 優先度分類

### 高優先度（実装ブロッカー）
1. プロジェクト保存/読み込み（§2）

### 中優先度（機能完全性）
2. エクスポート機能（§3）
3. メディア管理の改善（§4、残りはリロケーション提示・欠落時の扱い・大容量上限）
4. Script連携の詳細化（§5、残りはユーザー作成スクリプトの API と評価モデルが中心）
5. Nested Timelineのレンダリング（§6）
6. エラーハンドリングの残件（§11、レポート収集とユーザーガイダンス方針）
7. ネットワークの残件（§9、同時編集の意味論と切断クリーンアップの接続）

### 低優先度（将来拡張）
8. オーディオ処理（§7）
9. プラグインシステムの残件（§12、依存関係・互換性・配布）
10. 未固定の契約の明文化（§13、REFACTORING_AUDIT の RF-011）
11. ローカライゼーション拡張（§14）
12. パフォーマンス計測（§10、計測基盤の導入自体が判断事項）
13. ビルドとデプロイの残件（§15、自動更新と配布戦略）
14. GUI側の切断時の扱い（§8、内蔵サーバー前提での方針判断）

---

## 実装時の判断基準

AIエージェントが実装時に判断が必要な場合：

1. **既存パターンに従う**: 同様の機能が既に実装されている場合、そのパターンを模倣する
2. **最小限の実装**: 不明な点は最小限の機能で実装し、後で拡張可能にする
3. **ログ出力**: 不明な挙動はログを出力してデバッグ可能にする
4. **エラーハンドリング**: 不明なエラーは適切に処理し、ユーザーに通知する
5. **ドキュメント更新**: 実装した機能はこのドキュメントを更新する
