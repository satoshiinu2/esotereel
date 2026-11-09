use std::sync::{Arc, RwLock};

use dashmap::DashMap;

use crate::dirs::Directories;
use crate::network::NetworkHandler;
use crate::plugin::script::api::register_fn_for;
use crate::plugin::{PluginLoadedResult, PluginLoader};
use crate::project::Project;
use crate::util::result::LockExt;
use crate::{HostRole, StreamState};

/// アプリケーション共通のステートを保持する構造体です。
///
/// クライアント／サーバーの両ホストで共有されます。`project` と
/// `plugin_loader` は `Arc<RwLock<>>` で保護され、`stream_state_map` は
/// スレッドセーフな `DashMap` です。`script_engine` は内部サーバー間で
/// 共有されず、各ホストが個別に保持します。
pub struct CommonState {
    /// プロジェクトモデル。`Arc<RwLock<>>` で保護され、読み取り／書き込みは
    /// `LockExt` によりエラー処理付きで取得します。
    pub project: Arc<RwLock<Option<Project>>>,

    /// ディレクトリパス解決用のインスタンス。
    pub dir: Directories,

    /// パス → ストリーム状態のマップ（`DashMap` でスレッドセーフ）。
    pub stream_state_map: Arc<DashMap<String, StreamState>>,

    /// プラグインローダー。内部サーバーで共有される `Arc<RwLock<>>`。
    pub plugin_loader: Arc<RwLock<PluginLoader>>,

    /// Rhai スクリプトエンジン。内部サーバー間では共有されず、各ホストが個別に保持。
    pub script_engine: rhai::Engine,
}

impl CommonState {
    /// 新しい `CommonState` を作成します。
    ///
    /// `shared_plugin_loader` が指定された場合はそれを再利用し、そうでなければ
    /// 新規の `PluginLoader` を作成します。`HostState` に応じた Rhai 関数を
    /// `register_fn_for` で登録します。
    pub fn new<T: HostState>(
        dirs_def: Directories,
        shared_plugin_loader: Option<Arc<RwLock<PluginLoader>>>,
    ) -> Self {
        let plugin_loader =
            shared_plugin_loader.unwrap_or(Arc::new(RwLock::new(PluginLoader::new())));

        let mut script_engine = rhai::Engine::new();
        register_fn_for::<T>(&mut script_engine);

        Self {
            project: Arc::new(RwLock::new(None)),
            dir: dirs_def,
            stream_state_map: Arc::new(DashMap::new()),
            plugin_loader,
            script_engine,
        }
    }

    /// プラグイン共通スクリプトの関数を呼び出します。
    ///
    /// `plugin_loader` の読み取りロックを取得し、`PluginLoader::call_script` に委譲します。
    pub fn call_script<R: Clone + Send + Sync + 'static>(
        &self,
        plugin_id: &str,
        fn_name: &str,
        args: impl rhai::FuncArgs,
    ) -> anyhow::Result<R> {
        let loader = self.plugin_loader.read_or_err()?;
        loader.call_script::<R>(&self.script_engine, plugin_id, fn_name, args)
    }

    /// ディスクから全プラグインを非同期に読み込みます。
    ///
    /// `plugin_loader` の書き込みロックを取得し、`PluginLoader::load_from_disk` を実行します。
    pub async fn load_plugins(&self) -> anyhow::Result<Vec<PluginLoadedResult>> {
        let mut loader = self.plugin_loader.write_or_err()?;
        loader.load_from_disk(&self.dir).await
    }
}

/// ホスト固有のステートを表すトレイトです。
///
/// `CommonState` を `Deref` して共有ステートにアクセスし、`DerefMut` で
/// ホスト固有のフィールドを変更できます。`Client` と `Server` で
/// `ROLE` 定数と `NetworkHandler` 型を分岐させます。
pub trait HostState: std::ops::Deref<Target = CommonState> + std::ops::DerefMut {
    /// 使用するネットワークハンドラの型。
    type NetworkHandler: NetworkHandler;

    /// ホストの役割（`HostRole::Client` または `HostRole::Server`）。
    const ROLE: HostRole;

    /// 起動時にプラグイン読み込みと設定適用を行います。
    async fn boot_strap(&mut self) {
        if let Err(e) = self.load_plugins().await {
            log::error!("Failed to load plugins: {:?}", e);
        } else {
            log::info!("Plugins loaded successfully");
        }

        if let Err(e) = self.apply_plugin_fields() {
            log::error!("Failed to apply settings: {:?}", e);
        } else {
            log::info!("Settings applied successfully");
        }
    }

    /// クライアント／サーバー固有の追加フィールドを適用します。
    ///
    /// 何もしない場合はデフォルトのままとなります。サブトレイトでオーバーライド可能。
    fn apply_extra_plugin_fields(&mut self) -> anyhow::Result<()> {
        Ok(())
    }

    /// プラグインフィールドを適用します（デフォルトは `apply_extra_plugin_fields` のみ）。
    fn apply_plugin_fields(&mut self) -> anyhow::Result<()> {
        self.apply_extra_plugin_fields()
    }
}
