//! Minimal publish/subscribe event bus used to decouple UI actions from the git service.

use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Mutex, OnceLock};

use crate::app::actions::AppAction;

pub struct EventBus<T: Clone> {
    subscribers: Mutex<Vec<Sender<T>>>,
}

impl<T: Clone> EventBus<T> {
    pub const fn new() -> Self {
        Self {
            subscribers: Mutex::new(Vec::new()),
        }
    }

    /// Registers a new subscriber that receives every event published afterwards.
    pub fn subscribe(&self) -> Receiver<T> {
        let (tx, rx) = channel();
        self.subscribers.lock().unwrap().push(tx);
        rx
    }

    /// Delivers `event` to all live subscribers; dropped receivers are removed.
    pub fn publish(&self, event: T) {
        self.subscribers
            .lock()
            .unwrap()
            .retain(|subscriber| subscriber.send(event.clone()).is_ok());
    }
}

static BUS: OnceLock<EventBus<AppAction>> = OnceLock::new();

fn bus() -> &'static EventBus<AppAction> {
    BUS.get_or_init(EventBus::new)
}

pub fn subscribe() -> Receiver<AppAction> {
    bus().subscribe()
}

pub fn publish(action: AppAction) {
    bus().publish(action);
}

