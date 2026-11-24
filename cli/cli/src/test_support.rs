pub(crate) fn parsed(words: &[&str]) -> dx_plan::args::Invocation {
    dx_plan::args::parse(&dx_testing::cli::strings(words)).expect("parse")
}

const SETTLE_LIMIT: std::time::Duration = std::time::Duration::from_secs(1);

pub(crate) fn slot_settles_free(dir: &std::path::Path, count: usize) -> bool {
    settles_within(dir, count, SETTLE_LIMIT)
}

fn settles_within(dir: &std::path::Path, count: usize, limit: std::time::Duration) -> bool {
    (0..count).all(|index| {
        let path = dx_plan::slots::slot_path(dir, index);
        !path.exists() || dx_test_scratch::wait::lock_free(&path, limit).is_ok()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slot_settles_free_waits_for_a_late_release() {
        let scratch = dx_test_scratch::scratch("dx-settle-late-").unwrap();
        let dir = scratch.path().join("slots");
        let held = dx_plan::slots::try_take(&dir, 1, "held")
            .expect("try")
            .expect("free");
        let (sender, settled) = std::sync::mpsc::channel();
        let waiter = dir.clone();
        let settler = std::thread::spawn(move || {
            sender
                .send(settles_within(
                    &waiter,
                    1,
                    std::time::Duration::from_secs(30),
                ))
                .unwrap_or_default();
        });
        assert!(settled.try_recv().is_err(), "settled while still held");
        drop(held);
        assert_eq!(
            settled.recv_timeout(std::time::Duration::from_secs(30)),
            Ok(true)
        );
        settler.join().expect("join");
    }

    #[test]
    fn slot_settles_free_gives_up_on_a_held_slot() {
        let scratch = dx_test_scratch::scratch("dx-settle-held-").unwrap();
        let dir = scratch.path().join("slots");
        let _held = dx_plan::slots::try_take(&dir, 1, "held")
            .expect("try")
            .expect("free");
        assert!(!slot_settles_free(&dir, 1));
    }
}
