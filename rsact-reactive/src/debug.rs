use crate::{runtime::with_current_runtime, storage::ValueDebugInfo};

pub fn observer_debug_info() -> Option<ValueDebugInfo> {
    with_current_runtime(|rt| rt.observer_debug_info())
}
