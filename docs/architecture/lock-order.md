# ロック取得順序（Lock Order）

Esotereel は GUI スレッド・ネットワークスレッド・レンダースレッドが同じ
状態を触るため、ロックの取得順序が守られないと AB-BA 型デッドロックになる。
このドキュメントは全体で守るべき順序と禁止事項を定める。

## 前提: State 自体にロックは無い

`ClientState` / `ServerState` は外側のロックで包まれていない
（ハンドルは `Arc<ClientState>`、サーバー内部は `Arc<ServerState>`）。
可変状態は全てフィールド単位の同期プリミティブで保護する。

- `CommonState`: `project` / `plugin_loader` は `Arc<RwLock<…>>`、
  `stream_state_map` は `Arc<DashMap<…>>`、`script_engine` は `Arc<Engine>`
- クライアント固有: `settings` / `toolbar` は `Arc<RwLock<Store>>`、
  `stream_players` / `last_*_fetch` は `DashMap`、
  `async_error` は `Arc<Mutex<Option<String>>>`、
  `gui_callbacks` は生成後に不変な関数ポインタ
- サーバー固有: `histories` / `uploads` は `Arc<Mutex<…>>`、
  `streams` は `DashMap`、`next_resource_id` は `AtomicU32`

`HostState` のメソッドも `&self` のみを要求する。新しい可変状態を
追加する場合は、素のフィールドではなく上記のいずれかで保護し、
このドキュメントの順序に位置付けること。

## グローバル順序

複数ロックが必要な場合は、必ずこの順序でのみ取得すること。

1. Project RwLock（`Arc<RwLock<Option<Project>>>` の read / write）
2. PluginLoader RwLock（`Arc<RwLock<PluginLoader>>`）
3. histories Mutex（core の `Mutex<HashMap<TimelineId, HistoryStack>>`）
4. ストア／補助ロック（guihlp の settings / toolbar RwLock、
   `async_error` Mutex、core の uploads Mutex）
5. ネットワーク内部 RwLock（`tx`、`clients`、`client_views` 等）

逆順の取得は禁止。4 のロックは原則として単独で短時間だけ取得し、
1〜3 を保持したまま取得しない。特に GUI 側は Project の read ガード
（C++ `Project` RAII）を保持したまま、他のロックを取る FFI
（設定取得、クリップの共通フィールド取得など）を呼んではならない。
Project の write を取る処理（レスポンス適用など）とサイクルになる。

## 保持したまま呼んではいけないもの

以下の呼び出しは、Project をはじめとする上記ロックを保持したまま
行わないこと。

- GUI コールバック（`GuiCallbacks` の関数ポインタ呼び出し）
- ネットワーク送信（`network.send` / `send_to_many` 等）
- プラグインスクリプトの実行（Rhai の `call` / `call_script`）
  - スクリプトは `PluginActionContext::set_clip_property` 経由で
    Project の write ロックを取りうるため、Loader ロック保持中の
    実行も禁止する
  - `script_engine` は `Arc` なので、クローンを取り出せばどのロックも
    保持せずに実行できる
- デコード等の重い処理（`StreamPlayer::process_packet` 等）

## 推奨パターン（Phase 分離）

既存コードで使われている Phase1 / Phase2 パターンを標準とする。

1. Phase 1: ハンドルのアクセサ（`project_arc()` / `network_arc()` 等）で
   必要な `Arc` のクローンと、コールバック関数ポインタの値コピーだけを
   行う。
2. Phase 2: ロックなし、または Project ロックのみで本来の処理を行う。
3. Phase 3: コールバック呼び出し・ネットワーク送信は、すべての
   ガードを drop した後に行う。

## プロジェクトの差し替えについて

プロジェクトの差し替え（サーバーの NewProject、クライアントの
ProjectMeta 適用）は、`state.project` の `Arc` を付け替えるのではなく、
既存の RwLock へのインプレース書き込み
（`*project.write() = Some(…)`）で行う。
このため差し替えは、Project の read ガードを保持している処理
（GUI の描画、C++ へ渡した read ガード）の解放を待つ。
read ガードはフレーム単位の短命なものだけにすること。

## FFI の Project ガードについて

`client_state_project_lock_read` が C++ へ渡す read ガードは、
対象の `Arc<RwLock<Option<Project>>>` 自体をガード側で保持する。
これにより、ClientState が先に破棄されても RwLock が解放されず、
ガード経由の use-after-free になることはない（ISSUES.md 1.1 参照）。
