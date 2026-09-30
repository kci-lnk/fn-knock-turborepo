//! Page ownership is independent of the selected output attachment. Lock order:
//! session.api_operation -> pages -> session.state. Remove/cancel abandoned
//! sessions under these locks; wait for their actors only after releasing them.
use std::collections::HashSet;

use super::super::domain::TerminalPage;
use super::*;

const PAGE_TTL: Duration = Duration::from_secs(120);
const MAX_PAGES: usize = 1_024;

pub(super) struct PageLease {
    last_seen: Instant,
    sessions: HashSet<String>,
}

fn page_expired() -> TerminalError {
    TerminalError::new(TerminalErrorCode::Conflict, "terminal page lease expired")
}

fn valid_page<'a>(
    pages: &'a mut HashMap<String, PageLease>,
    id: &str,
) -> TerminalResult<&'a mut PageLease> {
    pages
        .get_mut(id)
        .filter(|page| page.last_seen.elapsed() < PAGE_TTL)
        .ok_or_else(page_expired)
}

pub(super) fn register_session_owner(
    pages: &mut HashMap<String, PageLease>,
    page_id: Option<&str>,
    session_id: &str,
) -> TerminalResult<()> {
    if let Some(id) = page_id {
        valid_page(pages, id)?
            .sessions
            .insert(session_id.to_string());
    }
    Ok(())
}

impl TerminalRuntime {
    pub async fn register_page(&self) -> TerminalResult<TerminalPage> {
        let mut pages = self.pages.lock().await;
        pages.retain(|_, page| page.last_seen.elapsed() < PAGE_TTL);
        if pages.len() >= MAX_PAGES {
            return Err(TerminalError::new(
                TerminalErrorCode::Conflict,
                "terminal page limit reached",
            ));
        }
        let id = Uuid::new_v4().to_string();
        pages.insert(
            id.clone(),
            PageLease {
                last_seen: Instant::now(),
                sessions: HashSet::new(),
            },
        );
        Ok(TerminalPage {
            id,
            expires_at: iso_after_seconds(PAGE_TTL.as_secs() as i64),
        })
    }

    pub async fn heartbeat_page(&self, id: &str) -> TerminalResult<TerminalPage> {
        let mut pages = self.pages.lock().await;
        valid_page(&mut pages, id)?.last_seen = Instant::now();
        Ok(TerminalPage {
            id: id.to_string(),
            expires_at: iso_after_seconds(PAGE_TTL.as_secs() as i64),
        })
    }

    pub async fn release_page(&self, id: &str) {
        self.pages.lock().await.remove(id);
        self.cleanup_pages().await;
    }

    pub(in super::super) async fn start_page_session(
        &self,
        startup: SessionStartup,
        persistent: bool,
        page_id: Option<&str>,
    ) -> TerminalResult<TerminalSession> {
        let mut pages = self.pages.lock().await;
        if let Some(id) = page_id {
            valid_page(&mut pages, id)?;
        }
        {
            let mut state = startup.pending.runtime.state.lock().await;
            state.session.persistent = persistent;
            state.had_owner = page_id.is_some();
        }
        let session = self.start_session(startup).await?;
        if let Some(id) = page_id {
            // The lease cannot be released while startup is registered.
            if let Some(page) = pages.get_mut(id) {
                page.sessions.insert(session.id.clone());
            }
        }
        Ok(session)
    }

    pub async fn create_page_attachment(
        &self,
        session_id: &str,
        cols: Option<u32>,
        rows: Option<u32>,
        page_id: Option<&str>,
    ) -> TerminalResult<TerminalAttachment> {
        self.create_attachment_inner(session_id, cols, rows, page_id)
            .await
    }

    pub(super) async fn cleanup_pages(&self) {
        let tasks = {
            let sessions = self
                .sessions
                .read()
                .await
                .values()
                .cloned()
                .collect::<Vec<_>>();
            let mut tasks = Vec::new();
            for session in sessions {
                let _api = session.api_operation.lock().await;
                let mut pages = self.pages.lock().await;
                pages.retain(|_, page| page.last_seen.elapsed() < PAGE_TTL);
                let mut state = session.state.lock().await;
                // A released page must stop controlling/output-polling a shell,
                // even when that shell itself is persistent.
                state.attachments.retain(|_, attachment| {
                    attachment
                        .page_id
                        .as_ref()
                        .is_none_or(|id| pages.contains_key(id))
                });
                state.expire_attachments();
                let id = state.session.id.clone();
                let has_page = pages.values().any(|page| page.sessions.contains(&id));
                let has_legacy_attachment = state.attachments.values().any(|a| a.page_id.is_none());
                if state.session.persistent
                    || !state.session.phase.is_active()
                    || has_page
                    || has_legacy_attachment
                    || (!state.had_owner && state.created.elapsed() < PAGE_TTL)
                {
                    continue;
                }
                session.cancel.cancel();
                self.sessions.write().await.remove(&id);
                if let Some(task) = self.actor_tasks.lock().await.remove(&id) {
                    tasks.push(AbortOnDropHandle::new(task));
                }
                tracing::info!(session_id = %id, "unowned terminal session terminated");
            }
            let mut pages = self.pages.lock().await;
            pages.retain(|_, page| page.last_seen.elapsed() < PAGE_TTL);
            let live_ids = self
                .sessions
                .read()
                .await
                .keys()
                .cloned()
                .collect::<HashSet<_>>();
            for page in pages.values_mut() {
                page.sessions.retain(|id| live_ids.contains(id));
            }
            tasks
        };
        for task in tasks {
            finish_session_task(task, SESSION_SHUTDOWN_TIMEOUT).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::system::terminal::shell::InteractiveShell;
    use async_trait::async_trait;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct TrackedShell(Arc<AtomicUsize>);
    #[async_trait]
    impl InteractiveShell for TrackedShell {
        async fn next_event(&mut self) -> ShellEvent {
            std::future::pending().await
        }
        async fn input(&mut self, _: Vec<u8>) -> TerminalResult<()> {
            Ok(())
        }
        async fn resize(&mut self, _: u32, _: u32) -> TerminalResult<()> {
            Ok(())
        }
        async fn close(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
        async fn disconnect(&mut self) {}
    }

    async fn ready_pending_session(runtime: &TerminalRuntime) -> PendingSession {
        let pending = runtime
            .begin_session("ssh-target".into(), "shell".into(), 80, 24)
            .await
            .unwrap();
        for phase in [
            SessionPhase::Resolving,
            SessionPhase::Connecting,
            SessionPhase::VerifyingHostKey,
            SessionPhase::Authenticating,
            SessionPhase::OpeningChannel,
            SessionPhase::RequestingPty,
            SessionPhase::Running,
        ] {
            set_phase(&pending.runtime, phase, None, None).await;
        }
        pending
    }

    async fn running(runtime: &TerminalRuntime) -> (TerminalSession, Arc<AtomicUsize>) {
        let pending = ready_pending_session(runtime).await;
        let closed = Arc::new(AtomicUsize::new(0));
        let session = runtime
            .activate_session(
                pending,
                Box::new(TrackedShell(closed.clone())),
                CancellationToken::new(),
            )
            .await
            .unwrap();
        (session, closed)
    }

    struct WaitingResize {
        started: Arc<Notify>,
        proceed: Arc<Notify>,
        closed: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl InteractiveShell for WaitingResize {
        async fn next_event(&mut self) -> ShellEvent {
            std::future::pending().await
        }
        async fn input(&mut self, _: Vec<u8>) -> TerminalResult<()> {
            Ok(())
        }
        async fn resize(&mut self, _: u32, _: u32) -> TerminalResult<()> {
            self.started.notify_one();
            self.proceed.notified().await;
            Ok(())
        }
        async fn close(&mut self) {
            self.closed.fetch_add(1, Ordering::SeqCst);
        }
        async fn disconnect(&mut self) {}
    }

    #[tokio::test]
    async fn slow_cancelled_attachment_never_blocks_heartbeats_or_loses_page_ownership() {
        let runtime = TerminalRuntime::new();
        let page = runtime.register_page().await.unwrap();
        let pending = ready_pending_session(&runtime).await;
        let started = Arc::new(Notify::new());
        let proceed = Arc::new(Notify::new());
        let closed = Arc::new(AtomicUsize::new(0));
        let session = runtime
            .activate_session(
                pending,
                Box::new(WaitingResize {
                    started: started.clone(),
                    proceed: proceed.clone(),
                    closed: closed.clone(),
                }),
                CancellationToken::new(),
            )
            .await
            .unwrap();
        runtime
            .update_session(&session.id, None, Some(false))
            .await
            .unwrap();
        let mut attach =
            Box::pin(runtime.create_page_attachment(&session.id, Some(100), None, Some(&page.id)));
        std::future::poll_fn(|cx| {
            assert!(std::future::Future::poll(attach.as_mut(), cx).is_pending());
            std::task::Poll::Ready(())
        })
        .await;
        started.notified().await;
        tokio::time::timeout(Duration::from_millis(200), runtime.heartbeat_page(&page.id))
            .await
            .expect("backend resize must not hold the shared lease lock")
            .unwrap();
        let mut cleanup = Box::pin(runtime.cleanup_pages());
        std::future::poll_fn(|cx| {
            assert!(std::future::Future::poll(cleanup.as_mut(), cx).is_pending());
            std::task::Poll::Ready(())
        })
        .await;
        tokio::time::timeout(Duration::from_millis(200), runtime.heartbeat_page(&page.id))
            .await
            .expect("maintenance waiting for a session must not block heartbeat")
            .unwrap();
        // An HTTP disconnect cancels the request during its resize response.
        drop(attach);
        proceed.notify_one();
        cleanup.await;
        assert_eq!(closed.load(Ordering::SeqCst), 0);
        assert_eq!(runtime.list().await.sessions.len(), 1);
        runtime.release_page(&page.id).await;
        assert_eq!(closed.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn persistent_by_default_and_release_revokes_attachment_without_killing() {
        let runtime = TerminalRuntime::new();
        let page = runtime.register_page().await.unwrap();
        let (session, closed) = running(&runtime).await;
        assert!(session.persistent);
        let attachment = runtime
            .create_page_attachment(&session.id, None, None, Some(&page.id))
            .await
            .unwrap();
        runtime.release_page(&page.id).await;
        runtime.release_page(&page.id).await;
        assert!(runtime.heartbeat_page(&page.id).await.is_err());
        assert!(
            runtime
                .create_page_attachment(&session.id, None, None, Some(&page.id))
                .await
                .is_err()
        );
        assert!(
            runtime
                .session_for_attachment(&attachment.id)
                .await
                .is_err()
        );
        assert_eq!(runtime.list().await.sessions.len(), 1);
        assert_eq!(closed.load(Ordering::SeqCst), 0);
        runtime.shutdown_all().await;
    }

    #[tokio::test]
    async fn connection_setting_controls_existing_processes_on_last_page_release() {
        let runtime = TerminalRuntime::new();
        let page = runtime.register_page().await.unwrap();
        let (first, first_closed) = running(&runtime).await;
        let (second, second_closed) = running(&runtime).await;
        for session in [&first, &second] {
            runtime
                .create_page_attachment(&session.id, None, None, Some(&page.id))
                .await
                .unwrap();
        }
        runtime.set_target_persistence("ssh-target", false).await;
        runtime.expire_attachments().await;
        assert_eq!(first_closed.load(Ordering::SeqCst), 0);
        assert_eq!(second_closed.load(Ordering::SeqCst), 0);
        runtime.set_target_persistence("ssh-target", true).await;
        runtime.release_page(&page.id).await;
        assert_eq!(first_closed.load(Ordering::SeqCst), 0);
        let page = runtime.register_page().await.unwrap();
        for session in [&first, &second] {
            runtime
                .create_page_attachment(&session.id, None, None, Some(&page.id))
                .await
                .unwrap();
        }
        runtime.set_target_persistence("ssh-target", false).await;
        runtime.release_page(&page.id).await;
        assert_eq!(first_closed.load(Ordering::SeqCst), 1);
        assert_eq!(second_closed.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn switching_sessions_keeps_both_until_last_page_releases() {
        let runtime = TerminalRuntime::new();
        let a = runtime.register_page().await.unwrap();
        let b = runtime.register_page().await.unwrap();
        let (first, first_closed) = running(&runtime).await;
        let (second, second_closed) = running(&runtime).await;
        for session in [&first, &second] {
            runtime
                .update_session(&session.id, None, Some(false))
                .await
                .unwrap();
            let attachment = runtime
                .create_page_attachment(&session.id, None, None, Some(&a.id))
                .await
                .unwrap();
            runtime.detach(&attachment.id).await.unwrap();
        }
        runtime
            .create_page_attachment(&first.id, None, None, Some(&b.id))
            .await
            .unwrap();
        runtime.expire_attachments().await;
        assert_eq!(runtime.list().await.sessions.len(), 2);
        runtime.release_page(&a.id).await;
        assert_eq!(second_closed.load(Ordering::SeqCst), 1);
        assert_eq!(first_closed.load(Ordering::SeqCst), 0);
        assert_eq!(runtime.list().await.sessions[0].id, first.id);
        runtime.release_page(&b.id).await;
        assert_eq!(first_closed.load(Ordering::SeqCst), 1);
        assert!(runtime.list().await.sessions.is_empty());
    }

    #[tokio::test]
    async fn expired_pages_do_not_revive_and_legacy_attachments_protect_sessions() {
        let runtime = TerminalRuntime::new();
        let page = runtime.register_page().await.unwrap();
        let (session, closed) = running(&runtime).await;
        runtime
            .create_page_attachment(&session.id, None, None, Some(&page.id))
            .await
            .unwrap();
        let legacy = runtime
            .create_attachment(&session.id, None, None)
            .await
            .unwrap();
        runtime
            .update_session(&session.id, None, Some(false))
            .await
            .unwrap();
        runtime
            .pages
            .lock()
            .await
            .get_mut(&page.id)
            .unwrap()
            .last_seen = Instant::now() - PAGE_TTL;
        assert!(runtime.heartbeat_page(&page.id).await.is_err());
        runtime.expire_attachments().await;
        assert_eq!(closed.load(Ordering::SeqCst), 0);
        let owned = runtime.session(&session.id).await.unwrap();
        owned
            .state
            .lock()
            .await
            .attachments
            .get_mut(&legacy.id)
            .unwrap()
            .last_seen = Instant::now() - PAGE_TTL;
        runtime.expire_attachments().await;
        assert_eq!(closed.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn cleanup_rechecks_persistence_and_new_owners_under_operation_lock() {
        let runtime = TerminalRuntime::new();
        let page = runtime.register_page().await.unwrap();
        let (session, closed) = running(&runtime).await;
        runtime
            .create_page_attachment(&session.id, None, None, Some(&page.id))
            .await
            .unwrap();
        runtime
            .update_session(&session.id, None, Some(false))
            .await
            .unwrap();
        let owned = runtime.session(&session.id).await.unwrap();
        let guard = owned.api_operation.lock().await;
        // Queue the toggle first, then removal. The cleanup must recheck the
        // latest persistence value after it eventually acquires the lock.
        let toggle = runtime.update_session(&session.id, None, Some(true));
        tokio::pin!(toggle);
        std::future::poll_fn(|cx| {
            assert!(std::future::Future::poll(toggle.as_mut(), cx).is_pending());
            std::task::Poll::Ready(())
        })
        .await;
        let release = runtime.release_page(&page.id);
        tokio::pin!(release);
        std::future::poll_fn(|cx| {
            assert!(std::future::Future::poll(release.as_mut(), cx).is_pending());
            std::task::Poll::Ready(())
        })
        .await;
        drop(guard);
        let (updated, ()) = tokio::join!(toggle, release);
        assert!(updated.unwrap().persistent);
        assert_eq!(closed.load(Ordering::SeqCst), 0);
        runtime.shutdown_all().await;
    }

    #[tokio::test]
    async fn concurrent_attachment_is_registered_before_last_page_cleanup() {
        let runtime = TerminalRuntime::new();
        let first = runtime.register_page().await.unwrap();
        let second = runtime.register_page().await.unwrap();
        let (session, closed) = running(&runtime).await;
        runtime
            .create_page_attachment(&session.id, None, None, Some(&first.id))
            .await
            .unwrap();
        runtime
            .update_session(&session.id, None, Some(false))
            .await
            .unwrap();
        let owned = runtime.session(&session.id).await.unwrap();
        let guard = owned.api_operation.lock().await;
        let attach = runtime.create_page_attachment(&session.id, None, None, Some(&second.id));
        tokio::pin!(attach);
        std::future::poll_fn(|cx| {
            assert!(std::future::Future::poll(attach.as_mut(), cx).is_pending());
            std::task::Poll::Ready(())
        })
        .await;
        let release = runtime.release_page(&first.id);
        tokio::pin!(release);
        std::future::poll_fn(|cx| {
            assert!(std::future::Future::poll(release.as_mut(), cx).is_pending());
            std::task::Poll::Ready(())
        })
        .await;
        drop(guard);
        let (attached, ()) = tokio::join!(attach, release);
        attached.unwrap();
        assert_eq!(closed.load(Ordering::SeqCst), 0);
        runtime.release_page(&second.id).await;
        assert_eq!(closed.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn heartbeat_extends_live_lease_but_cannot_undo_concurrent_release() {
        let runtime = TerminalRuntime::new();
        let page = runtime.register_page().await.unwrap();
        let (session, closed) = running(&runtime).await;
        runtime
            .create_page_attachment(&session.id, None, None, Some(&page.id))
            .await
            .unwrap();
        runtime
            .update_session(&session.id, None, Some(false))
            .await
            .unwrap();
        runtime
            .pages
            .lock()
            .await
            .get_mut(&page.id)
            .unwrap()
            .last_seen = Instant::now() - PAGE_TTL + Duration::from_secs(1);
        runtime.heartbeat_page(&page.id).await.unwrap();
        runtime.expire_attachments().await;
        assert_eq!(closed.load(Ordering::SeqCst), 0);
        let _ = tokio::join!(
            runtime.heartbeat_page(&page.id),
            runtime.release_page(&page.id)
        );
        assert!(runtime.heartbeat_page(&page.id).await.is_err());
        assert_eq!(closed.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn unattached_nonpersistent_creation_has_bounded_grace_and_patch_preserves_title() {
        let runtime = TerminalRuntime::new();
        let (session, closed) = running(&runtime).await;
        let changed = runtime
            .update_session(&session.id, None, Some(false))
            .await
            .unwrap();
        assert_eq!(changed.title, session.title);
        let renamed = runtime
            .update_session(&session.id, Some("renamed"), None)
            .await
            .unwrap();
        assert!(!renamed.persistent);
        runtime.expire_attachments().await;
        assert_eq!(closed.load(Ordering::SeqCst), 0);
        runtime
            .session(&session.id)
            .await
            .unwrap()
            .state
            .lock()
            .await
            .created = Instant::now() - PAGE_TTL;
        runtime.expire_attachments().await;
        assert_eq!(closed.load(Ordering::SeqCst), 1);
        assert!(
            runtime
                .update_session(&session.id, None, Some(true))
                .await
                .is_err()
        );
    }
}
