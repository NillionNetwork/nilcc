use chrono::Utc;
use cvm_agent_models::health::{EventKind, LastEvent};
use std::sync::{Arc, Mutex};

pub(crate) mod caddy;
pub(crate) mod compose;

#[derive(Clone, Default)]
pub struct EventHolder(Arc<Mutex<Option<LastEvent>>>);

impl EventHolder {
    pub(crate) fn set<S: Into<String>>(&self, message: S, kind: EventKind) {
        let mut inner = self.0.lock().expect("lock poisoned");
        let id = inner.as_ref().map(|e| e.id.wrapping_add(1)).unwrap_or_default();
        *inner = Some(LastEvent { id, message: message.into(), kind, timestamp: Utc::now() });
    }

    pub(crate) fn get(&self) -> Option<LastEvent> {
        self.0.lock().expect("lock poisoned").clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set() {
        let holder = EventHolder::default();
        assert!(holder.get().is_none());

        holder.set("beep", EventKind::Error);
        assert_eq!(holder.get().unwrap().id, 0);

        holder.set("boop", EventKind::Warning);
        let event = holder.get().unwrap();
        assert_eq!(event.id, 1);
        assert_eq!(event.message, "boop");
        assert_eq!(event.kind, EventKind::Warning);
    }
}
