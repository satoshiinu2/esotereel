//! オートセーブの定期実行と、滞留チャンクのアンロードパスです。
//!
//! 変更は `on_project_event` が `project_dirty` を立てることで伝わり、
//! このループが設定の間隔ごとに世代を `data_dir/autosave/` へ書きます
//! （保存済み・未保存の区別なく同じ扱い）。世代の実処理は lib の
//! `autosave` API に委ねます。
//!
//! 同じ周期で、滞留チャンクのアンロード判定も回します。閾値は設定
//! （`chunk.unload_after_secs` / `chunk.keep_frequently_used`）から
//! `ChunkResidencyConfig` へ供給します。

use std::{
    sync::{Arc, atomic::Ordering},
    time::Duration,
};

use esotereel_lib::{
    project::{autosave::write_autosave_generation, chunk::ChunkResidencyConfig},
    util::result::LockExt,
};

use crate::{save, state::ServerState};

/// オートセーブの定期ループです。サーバー起動時に spawn します。
pub async fn autosave_loop(state: Arc<ServerState>) {
    loop {
        let interval = save::load_app_settings(&state)
            .autosave_interval_secs
            .max(5);
        tokio::time::sleep(Duration::from_secs(interval)).await;
        let tick_state = Arc::clone(&state);
        let _ = tokio::task::spawn_blocking(move || run_autosave_tick(&tick_state)).await;
    }
}

fn run_autosave_tick(state: &ServerState) {
    let settings = save::load_app_settings(state);
    unload_idle_chunks(state, &settings);
    write_generation_if_dirty(state, &settings);
}

/// 設定から供給した方針で、滞留チャンクをアンロードします。
/// 書き出しは各タイムラインのチャンクストア（プロジェクトフォルダ）
/// へ行われます。
fn unload_idle_chunks(state: &ServerState, settings: &esotereel_lib::settings::AppSettings) {
    let config = ChunkResidencyConfig {
        idle_timeout: Duration::from_secs(settings.chunk_unload_after_secs),
        frequent_access_threshold: if settings.chunk_keep_frequently_used {
            ChunkResidencyConfig::default().frequent_access_threshold
        } else {
            0
        },
    };
    let Ok(mut project_guard) = state.project.write_or_err() else {
        return;
    };
    let Some(project) = project_guard.as_mut() else {
        return;
    };
    let timeline_ids: Vec<_> = project.iter_timelines().map(|(id, _)| *id).collect();
    for timeline_id in timeline_ids {
        let Some(timeline) = project.timeline_mut(timeline_id) else {
            continue;
        };
        match timeline.unload_idle_chunks(&config) {
            Ok(0) => {}
            Ok(count) => {
                log::info!(
                    "autosave tick: unloaded {count} idle chunk(s) of timeline {timeline_id}"
                );
            }
            Err(error) => {
                log::error!(
                    "autosave tick: failed to unload idle chunks of timeline {timeline_id}: {error:#}"
                );
            }
        }
    }
}

/// 変更が溜まっていればオートセーブ世代を1つ書きます。
fn write_generation_if_dirty(state: &ServerState, settings: &esotereel_lib::settings::AppSettings) {
    // 先にフラグを下ろし、書き込み中に来た変更は次の周期で拾います。
    // 失敗したらフラグを戻して再試行させます。
    if !state.project_dirty.swap(false, Ordering::SeqCst) {
        return;
    }
    // ? 演算子による早期リターンの範囲を限定するためクロージャーを使用
    let result = (|| -> anyhow::Result<()> {
        let data_dir = state.dir.data_dir()?;
        let mut project_guard = state.project.write_or_err()?;
        let project = project_guard
            .as_mut()
            .ok_or(esotereel_lib::util::result::EsotereelError::ProjectNotFound)?;
        project.ensure_all_chunks_loaded()?;
        let generation = write_autosave_generation(&data_dir, project, settings)?;
        log::info!(
            "autosave: generation {} written for project {} ({} bytes)",
            generation.id,
            generation.project_id,
            generation.size_bytes
        );
        Ok(())
    })();
    if let Err(error) = result {
        log::error!("autosave failed: {error:#}");
        state.project_dirty.store(true, Ordering::SeqCst);
    }
}
