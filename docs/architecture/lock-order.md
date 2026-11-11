# ロック取得順序（Lock Order）

Esotereel は GUI スレッド・ネットワークスレッド・レンダースレッドが同じ
状態を触るため、ロックの取得順序が守られないと AB-BA 型デッドロックになる。
このドキュメントは全体で守るべき順序と禁止事項を定める。

## グローバル順序

複数ロックが必要な場合は、必ずこの順序でのみ取得すること。

1. State Mutex（`Mutex<ClientState>` / `Mutex<ServerState>`）
2. Project RwLock（`Arc<RwLock<Option<Project>>>` の read / write）
3. PluginLoader RwLock（`Arc<RwLock<PluginLoader>>`）
4. histories Mutex（core の `Mutex<HashMap<TimelineId, HistoryStack>>`）
5. ネットワーク内部 RwLock（`tx`、`clients`、`client_views` 等）

逆順の取得は禁止。特に GUI 側は Project の read ガード
（C++ `Project` RAII）を保持したまま State Mutex を取る FFI
（設定取得、クリップの共通フィールド取得など）を呼んではならない。
ネットワークスレッド側が State → Project の順で取っているため、
Project → State はそのままデッドロックのサイクルになる。

## 保持したまま呼んではいけないもの

以下の呼び出しは、State / Project のロックを保持したまま行わないこと。

- GUI コールバック（`GuiCallbacks` の関数ポインタ呼び出し）
- ネットワーク送信（`network.send` / `send_to_many` 等）
- プラグインスクリプトの実行（Rhai の `call` / `call_script`）
  - スクリプトは `PluginActionContext::set_clip_property` 経由で
    Project の write ロックを取りうるため、Loader ロック保持中の
    実行も禁止する
- デコード等の重い処理（`StreamPlayer::process_packet` 等）

## 推奨パターン（Phase 分離）

既存コードで使われている Phase1 / Phase2 パターンを標準とする。

1. Phase 1: State を短時間ロックし、必要な `Arc`
   （project / network / stream_players 等）のクローンと、
   コールバック関数ポインタの値コピーだけを行い、ガードを drop する。
2. Phase 2: ロックなし、または Project ロックのみで本来の処理を行う。
3. Phase 3: コールバック呼び出し・ネットワーク送信は、すべての
   ガードを drop した後に行う。

送信・呼び出し側も同様に、ネットワークやプラグインローダーの
`Arc` をクローンしてからガードを drop し、その後に送信・実行する。

## FFI の Project ガードについて

`client_state_project_lock_read` が C++ へ渡す read ガードは、
対象の `Arc<RwLock<Option<Project>>>` 自体をガード側で保持する。
これにより、サーバーから ProjectMeta が届いて `state.project` が
差し替えられても、旧 Project の RwLock が解放済みガード経由で
use-after-free になることはない（ISSUES.md 1.1 参照）。
