/// Parse test words into one invocation, panicking on a failure.
pub(crate) fn parsed(words: &[&str]) -> dx_plan::args::Invocation {
    dx_plan::args::parse(&dx_testing::cli::strings(words)).expect("parse")
}

const SETTLE_LIMIT: std::time::Duration = std::time::Duration::from_secs(1);
const SETTLE_POLL: std::time::Duration = std::time::Duration::from_millis(10);

/// Waits for `count` slots to look free, since a forked child can briefly hold a released slot's lock.
pub(crate) fn slot_settles_free(dir: &std::path::Path, count: usize) -> bool {
    let start = std::time::Instant::now();
    loop {
        if dx_plan::slots::try_take(dir, count, "settle")
            .expect("try")
            .is_some()
        {
            return true;
        }
        if start.elapsed() >= SETTLE_LIMIT {
            return false;
        }
        std::thread::sleep(SETTLE_POLL);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slot_settles_free_waits_for_a_late_release() {
        let scratch = dx_test_scratch::scratch("dx-settle-late-");
        let dir = scratch.path().join("slots");
        let held = dx_plan::slots::try_take(&dir, 1, "held")
            .expect("try")
            .expect("free");
        let releaser = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(50));
            drop(held);
        });
        assert!(slot_settles_free(&dir, 1));
        releaser.join().expect("join");
    }

    #[test]
    fn slot_settles_free_gives_up_on_a_held_slot() {
        let scratch = dx_test_scratch::scratch("dx-settle-held-");
        let dir = scratch.path().join("slots");
        let _held = dx_plan::slots::try_take(&dir, 1, "held")
            .expect("try")
            .expect("free");
        assert!(!slot_settles_free(&dir, 1));
    }
}
