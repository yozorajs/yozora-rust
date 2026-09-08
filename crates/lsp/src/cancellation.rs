use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::protocol::ResponseError;

/// Each task has its own flag. The server cancels; workers only check it between
/// analysis stages. The flag carries no other data that needs synchronization.
#[derive(Clone, Default)]
pub struct Cancellation(Arc<AtomicBool>);

impl Cancellation {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Relaxed);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }

    pub fn check(&self) -> Result<(), ResponseError> {
        if self.is_cancelled() {
            Err(ResponseError::new(-32800, "analysis cancelled"))
        } else {
            Ok(())
        }
    }
}
