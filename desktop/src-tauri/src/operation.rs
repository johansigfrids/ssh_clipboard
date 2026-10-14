//! Independent operation control: cancellation never waits on the session/transport lock.
use serde::Serialize;
use std::future::Future;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::watch;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct View {
    pub generation: String,
    pub active: Option<Active>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Active {
    pub id: String,
    pub label: &'static str,
    pub cancellable: bool,
    pub cancelling: bool,
}

struct Running {
    id: u64,
    label: &'static str,
    finishing: bool,
    cancel_requested: bool,
    mutation_started: bool,
}

#[derive(Default)]
struct State {
    generation: u64,
    next_id: u64,
    active: Option<Running>,
}

struct Inner {
    state: Mutex<State>,
    changed: watch::Sender<u64>,
}

#[derive(Clone)]
pub struct Controller(Arc<Inner>);

pub struct Operation {
    controller: Controller,
    id: u64,
    cancelled: AtomicBool,
}

impl Default for Controller {
    fn default() -> Self {
        let (changed, _) = watch::channel(0);
        Self(Arc::new(Inner {
            state: Mutex::new(State::default()),
            changed,
        }))
    }
}

impl Controller {
    pub fn subscribe(&self) -> watch::Receiver<u64> {
        self.0.changed.subscribe()
    }

    pub fn view(&self) -> Result<View, String> {
        let state = self
            .0
            .state
            .lock()
            .map_err(|_| "Operation state unavailable.")?;
        Ok(View {
            generation: state.generation.to_string(),
            active: state.active.as_ref().map(|active| Active {
                id: active.id.to_string(),
                label: active.label,
                cancellable: !active.finishing && !active.cancel_requested,
                cancelling: active.cancel_requested,
            }),
        })
    }

    fn changed(&self, state: &mut State) {
        state.generation += 1;
        self.0.changed.send_replace(state.generation);
    }

    pub fn begin(&self, label: &'static str) -> Result<Operation, String> {
        let mut state = self
            .0
            .state
            .lock()
            .map_err(|_| "Operation state unavailable.")?;
        if state.active.is_some() {
            return Err("Another operation is running.".into());
        }
        // Reserve enough generations for all transitions; never wrap identity/version IPC.
        if state.next_id == u64::MAX || state.generation > u64::MAX - 16 {
            return Err("Reopen the app to reset operation tracking.".into());
        }
        state.next_id += 1;
        let id = state.next_id;
        state.active = Some(Running {
            id,
            label,
            finishing: false,
            cancel_requested: false,
            mutation_started: false,
        });
        self.changed(&mut state);
        Ok(Operation {
            controller: self.clone(),
            id,
            cancelled: AtomicBool::new(false),
        })
    }

    pub fn cancel(&self, expected_id: &str) -> Result<View, String> {
        let mut state = self
            .0
            .state
            .lock()
            .map_err(|_| "Operation state unavailable.")?;
        let active = state
            .active
            .as_mut()
            .ok_or("This operation has already finished.")?;
        if active.id.to_string() != expected_id {
            return Err(
                "A different operation is running. Check the panel before cancelling.".into(),
            );
        }
        if active.finishing {
            return Err("This operation is already finishing and cannot be cancelled.".into());
        }
        if !active.cancel_requested {
            active.cancel_requested = true;
            self.changed(&mut state);
        }
        drop(state);
        self.view()
    }
}

impl Operation {
    fn update(
        &self,
        change: impl FnOnce(&mut Running) -> Result<(), String>,
    ) -> Result<(), String> {
        let mut state = self
            .controller
            .0
            .state
            .lock()
            .map_err(|_| "Operation state unavailable.")?;
        let active = state
            .active
            .as_mut()
            .filter(|active| active.id == self.id)
            .ok_or("Operation finished.")?;
        change(active)?;
        self.controller.changed(&mut state);
        Ok(())
    }

    pub fn mutation_started(&self) -> Result<(), String> {
        self.update(|active| {
            if active.cancel_requested {
                return Err("Cancelled before sending the change.".into());
            }
            active.mutation_started = true;
            Ok(())
        })
    }

    /// A native write is synchronous once posted. Arbitrate cancellation BEFORE posting it.
    pub fn finishing(&self) -> Result<(), String> {
        self.update(|active| {
            if active.cancel_requested {
                return Err("Cancelled before changing this computer's clipboard.".into());
            }
            active.finishing = true;
            Ok(())
        })
    }

    /// A validated server acknowledgment wins over a concurrent late Cancel request.
    pub fn confirmed(&self) -> Result<(), String> {
        self.update(|active| {
            active.cancel_requested = false;
            active.finishing = true;
            Ok(())
        })
    }

    pub fn was_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }

    async fn cancellation(&self) {
        let mut changed = self.controller.subscribe();
        loop {
            let cancelled = self
                .controller
                .0
                .state
                .lock()
                .map(|state| {
                    state
                        .active
                        .as_ref()
                        .is_some_and(|active| active.id == self.id && active.cancel_requested)
                })
                .unwrap_or(true);
            if cancelled {
                return;
            }
            if changed.changed().await.is_err() {
                return;
            }
        }
    }

    pub async fn run<T>(
        &self,
        future: impl Future<Output = Result<T, String>>,
    ) -> Result<T, String> {
        tokio::select! {
            biased;
            result = future => result,
            () = self.cancellation() => {
                self.cancelled.store(true, Ordering::Release);
                let uncertain = self.controller.0.state.lock().map(|state| state.active.as_ref().is_some_and(|active| active.id == self.id && active.mutation_started)).unwrap_or(true);
                Err(if uncertain {
                    "Stopped waiting. The server may already have applied the change. Refresh to check; nothing will be retried automatically."
                } else { "Cancelled. This computer's clipboard was left unchanged." }.into())
            }
        }
    }
}

impl Drop for Operation {
    fn drop(&mut self) {
        if let Ok(mut state) = self.controller.0.state.lock()
            && state
                .active
                .as_ref()
                .is_some_and(|active| active.id == self.id)
        {
            state.active = None;
            self.controller.changed(&mut state);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn cancellation_wakes_an_already_waiting_operation_without_the_session_lock() {
        let controller = Controller::default();
        let operation = controller.begin("Refresh").unwrap();
        let id = controller.view().unwrap().active.unwrap().id;
        let (started, receive_started) = tokio::sync::oneshot::channel();
        let running = operation.run(async {
            let _ = started.send(());
            std::future::pending::<Result<(), String>>().await
        });
        tokio::pin!(running);
        tokio::select! {
            result = &mut running => panic!("must still be waiting: {result:?}"),
            result = receive_started => result.unwrap(),
        }
        controller.cancel(&id).unwrap();
        let error = tokio::time::timeout(std::time::Duration::from_secs(1), &mut running)
            .await
            .unwrap()
            .unwrap_err();
        assert!(error.contains("left unchanged"));
        assert!(operation.was_cancelled());
    }

    #[tokio::test]
    async fn cancel_drops_pending_work_and_never_cancels_a_later_operation() {
        struct Dropped(Arc<AtomicBool>);
        impl Drop for Dropped {
            fn drop(&mut self) {
                self.0.store(true, Ordering::SeqCst);
            }
        }
        let controller = Controller::default();
        let operation = controller.begin("Refresh").unwrap();
        let first = controller.view().unwrap().active.unwrap().id;
        let dropped = Arc::new(AtomicBool::new(false));
        let guard = Dropped(dropped.clone());
        controller.cancel(&first).unwrap();
        let error = operation
            .run(async {
                let _guard = guard;
                std::future::pending::<Result<(), String>>().await
            })
            .await
            .unwrap_err();
        assert!(error.contains("left unchanged"));
        assert!(dropped.load(Ordering::SeqCst));
        assert!(operation.was_cancelled());
        drop(operation);
        let _next = controller.begin("Copy").unwrap();
        assert!(
            controller
                .cancel(&first)
                .unwrap_err()
                .contains("different operation")
        );
        assert!(!controller.view().unwrap().active.unwrap().cancelling);
    }

    #[tokio::test]
    async fn acknowledged_mutation_and_native_write_cannot_be_reported_as_cancelled() {
        let controller = Controller::default();
        let operation = controller.begin("Share").unwrap();
        operation.mutation_started().unwrap();
        controller
            .cancel(&controller.view().unwrap().active.unwrap().id)
            .unwrap();
        let result = operation
            .run(async {
                operation.confirmed()?;
                Ok(())
            })
            .await;
        assert!(result.is_ok());
        assert!(!operation.was_cancelled());
        assert!(!controller.view().unwrap().active.unwrap().cancellable);
        assert!(
            controller
                .cancel(&controller.view().unwrap().active.unwrap().id)
                .is_err()
        );
        drop(operation);
        let operation = controller.begin("Copy").unwrap();
        operation.finishing().unwrap();
        assert!(
            controller
                .cancel(&controller.view().unwrap().active.unwrap().id)
                .is_err()
        );
        assert!(operation.run(async { Ok(()) }).await.is_ok());
    }

    #[tokio::test]
    async fn lost_mutation_acknowledgment_is_uncertain_and_cancel_before_write_prevents_posting() {
        let controller = Controller::default();
        let operation = controller.begin("Share").unwrap();
        operation.mutation_started().unwrap();
        controller
            .cancel(&controller.view().unwrap().active.unwrap().id)
            .unwrap();
        assert!(
            operation
                .run(std::future::pending::<Result<(), String>>())
                .await
                .unwrap_err()
                .contains("may already have applied")
        );
        drop(operation);
        let operation = controller.begin("Copy").unwrap();
        controller
            .cancel(&controller.view().unwrap().active.unwrap().id)
            .unwrap();
        assert!(operation.finishing().is_err());
        assert!(operation.mutation_started().is_err());
    }
}
