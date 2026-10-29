// The probe's count of tooltips, the shulker boxes among them.
use std::sync::atomic::{AtomicU64, Ordering};

pub static SEEN: AtomicU64 = AtomicU64::new(0);
pub static SHULKER_BOXES: AtomicU64 = AtomicU64::new(0);

pub fn seen(item: &str) {
    SEEN.fetch_add(1, Ordering::Relaxed);
    if item.ends_with("shulker_box") {
        SHULKER_BOXES.fetch_add(1, Ordering::Relaxed);
    }
}
