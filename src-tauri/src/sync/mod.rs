//! 同期エンジン。単一タスクでの直列実行を保証し、進捗を Tauri イベントで push する。
//!
//! - `sync_now`: 差分同期（先頭ページから、既知 URI or 上限 5 ページで打ち切り）
//! - `start_initial_sync`: 初回同期（既定 30 日 or 30 ページまで遡る、中断可能）
//! - イベント: `sync:progress` / `sync:completed` / `sync:error` / `ratelimit:throttled`

pub mod extractor;
pub mod poller;

use crate::auth::SessionManager;
use crate::db::Db;
use crate::error::AppError;
use chrono::Utc;
use poller::{PollContext, SyncMode};
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};

/// 差分同期の上限ページ数（DESIGN §6.2）。
const DIFF_MAX_PAGES: u32 = 5;
/// 初回同期の 1 回あたりの上限ページ数（安全弁）。
/// 活発なアカウントでは 30 ページ ≈ 数時間分にしかならないため（learnings.md L3 実測）、
/// cursor レジューム（`sync_state.cursor`）と組み合わせ、複数回の実行で cutoff まで遡る。
/// 1 回で cutoff に届かなくても保存済み cursor から継続する。
const INITIAL_MAX_PAGES: u32 = 2000;
/// 初回遡りの既定日数（learnings.md L3。設定連動は Phase 5）。
pub const DEFAULT_INITIAL_DAYS: u32 = 30;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SyncPhase {
    Idle,
    Diff,
    Initial,
}

/// フロントへ返す・イベントで push する同期状態。
#[derive(Debug, Clone, Serialize)]
pub struct SyncStatus {
    pub running: bool,
    pub phase: SyncPhase,
    pub page: u32,
    #[serde(rename = "mediaAdded")]
    pub media_added: u32,
    #[serde(rename = "lastRunAt")]
    pub last_run_at: Option<i64>,
    #[serde(rename = "lastError")]
    pub last_error: Option<String>,
    #[serde(rename = "oldestIndexedAt")]
    pub oldest_indexed_at: Option<i64>,
    #[serde(rename = "throttledUntil")]
    pub throttled_until: Option<i64>,
    pub cancelled: bool,
}

impl Default for SyncStatus {
    fn default() -> Self {
        SyncStatus {
            running: false,
            phase: SyncPhase::Idle,
            page: 0,
            media_added: 0,
            last_run_at: None,
            last_error: None,
            oldest_indexed_at: None,
            throttled_until: None,
            cancelled: false,
        }
    }
}

/// 実行時の共有状態（running / cancel フラグと status）。
pub struct SyncRuntime {
    running: AtomicBool,
    cancel: AtomicBool,
    status: Mutex<SyncStatus>,
}

impl SyncRuntime {
    fn new() -> Self {
        SyncRuntime {
            running: AtomicBool::new(false),
            cancel: AtomicBool::new(false),
            status: Mutex::new(SyncStatus::default()),
        }
    }

    /// 直列実行の関門。既に走っていれば false。
    fn try_start(&self, phase: SyncPhase) -> bool {
        if self
            .running
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return false;
        }
        self.cancel.store(false, Ordering::SeqCst);
        let mut s = self.status.lock().unwrap();
        *s = SyncStatus {
            running: true,
            phase,
            last_run_at: s.last_run_at,
            ..SyncStatus::default()
        };
        true
    }

    fn finish(&self) {
        self.running.store(false, Ordering::SeqCst);
        self.update(|s| {
            s.running = false;
            s.phase = SyncPhase::Idle;
            s.throttled_until = None;
        });
    }

    pub fn begin(&self, phase: SyncPhase) {
        self.update(|s| s.phase = phase);
    }

    pub fn request_cancel(&self) {
        self.cancel.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancel.load(Ordering::SeqCst)
    }

    pub fn update(&self, f: impl FnOnce(&mut SyncStatus)) {
        let mut s = self.status.lock().unwrap();
        f(&mut s);
    }

    pub fn snapshot(&self) -> SyncStatus {
        self.status.lock().unwrap().clone()
    }
}

/// Tauri state として保持する同期エンジン。
pub struct Syncer {
    db: Arc<Db>,
    session: Arc<SessionManager>,
    runtime: Arc<SyncRuntime>,
}

impl Syncer {
    pub fn new(db: Arc<Db>, session: Arc<SessionManager>) -> Self {
        Syncer {
            db,
            session,
            runtime: Arc::new(SyncRuntime::new()),
        }
    }

    /// 差分同期を開始する。
    pub fn sync_now(&self, app: AppHandle) -> Result<(), AppError> {
        self.spawn(app, SyncMode::Differential {
            max_pages: DIFF_MAX_PAGES,
        })
    }

    /// 初回同期を開始する（days 日遡る）。
    pub fn start_initial_sync(&self, app: AppHandle, days: u32) -> Result<(), AppError> {
        let cutoff_ts = Utc::now().timestamp() - (days as i64) * 86_400;
        self.spawn(app, SyncMode::Initial {
            cutoff_ts,
            max_pages: INITIAL_MAX_PAGES,
        })
    }

    /// 実行中の同期を中断要求する。
    pub fn cancel(&self) {
        self.runtime.request_cancel();
    }

    /// 現在の同期状態。
    pub fn status(&self) -> SyncStatus {
        self.runtime.snapshot()
    }

    /// 共通の起動処理。ログイン確認 → 直列関門 → バックグラウンドタスク spawn。
    fn spawn(&self, app: AppHandle, mode: SyncMode) -> Result<(), AppError> {
        if self.session.current().is_none() {
            return Err(AppError::NotLoggedIn);
        }
        if !self.runtime.try_start(mode_phase(mode)) {
            // 既に実行中: 多重起動を防ぐ（DESIGN §6.3）。現状維持で戻す。
            return Ok(());
        }

        let ctx = PollContext {
            db: self.db.clone(),
            session: self.session.clone(),
            client: self.session.client(),
            runtime: self.runtime.clone(),
            app: app.clone(),
        };

        tauri::async_runtime::spawn(async move {
            let result = poller::run(&ctx, mode).await;
            ctx.runtime.finish();

            match result {
                Ok(outcome) => {
                    let now = Utc::now().timestamp();
                    ctx.runtime.update(|s| {
                        s.last_run_at = Some(now);
                        s.cancelled = outcome.cancelled;
                        s.page = outcome.pages;
                        s.media_added = outcome.media_added;
                    });
                    let _ = ctx.app.emit("sync:completed", ctx.runtime.snapshot());
                }
                Err(AppError::Cancelled) => {
                    ctx.runtime.update(|s| {
                        s.cancelled = true;
                        s.last_run_at = Some(Utc::now().timestamp());
                    });
                    let _ = ctx.app.emit("sync:completed", ctx.runtime.snapshot());
                }
                Err(e) => {
                    let msg = e.to_string();
                    tracing::warn!("同期エラー: {msg}");
                    ctx.runtime.update(|s| s.last_error = Some(msg.clone()));
                    let _ = ctx
                        .app
                        .emit("sync:error", serde_json::json!({ "message": msg }));
                }
            }
        });

        Ok(())
    }
}

fn mode_phase(mode: SyncMode) -> SyncPhase {
    match mode {
        SyncMode::Differential { .. } => SyncPhase::Diff,
        SyncMode::Initial { .. } => SyncPhase::Initial,
    }
}
