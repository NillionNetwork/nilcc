use chrono::Utc;
use cvm_agent_models::health::LastError;
use std::sync::{Arc, Mutex};

pub(crate) mod caddy;
pub(crate) mod compose;

#[derive(Clone, Default)]
pub struct ErrorHolder(Arc<Mutex<Option<LastError>>>);

impl ErrorHolder {
    pub(crate) fn set<S: Into<String>>(&self, message: S) {
        let mut inner = self.0.lock().expect("lock poisoned");
        let error_id = inner.as_ref().map(|e| e.error_id.wrapping_add(1)).unwrap_or_default();
        *inner = Some(LastError { message: message.into(), failed_at: Utc::now(), error_id });
    }

    pub(crate) fn get(&self) -> Option<LastError> {
        self.0.lock().expect("lock poisoned").clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set() {
        let holder = ErrorHolder::default();
        assert!(holder.get().is_none());

        holder.set("beep");
        assert_eq!(holder.get().unwrap().error_id, 0);

        holder.set("boop");
        let err = holder.get().unwrap();
        assert_eq!(err.error_id, 1);
        assert_eq!(err.message, "boop");
    }
}
