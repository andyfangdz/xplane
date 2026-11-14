//! Cheap identity observations for checks that still run on every operation.
#![forbid(unsafe_code)]

use std::thread::{self, ThreadId};

thread_local! {
    static CURRENT_ID: ThreadId = thread::current().id();
}

/// Returns the calling thread's Rust identifier.
///
/// Each thread copies its immutable ID once into thread-local storage. Repeated
/// calls avoid obtaining a temporary [`thread::Thread`] handle. Callers still
/// compare the returned ID with their recorded owner on every operation.
#[inline]
pub fn current_thread_id() -> ThreadId {
    CURRENT_ID.with(|id| *id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn cached_observations_match_each_threads_handle_and_reject_the_owner() {
        let owner = current_thread_id();
        assert_eq!(owner, thread::current().id());
        let children: Vec<_> = (0..8)
            .map(|_| {
                thread::spawn(|| {
                    let first = current_thread_id();
                    assert_eq!(first, thread::current().id());
                    assert_eq!(first, current_thread_id());
                    first
                })
            })
            .collect();
        let mut seen = HashSet::from([owner]);
        for child in children {
            let expected = child.thread().id();
            let observed = child.join().unwrap();
            assert_eq!(observed, expected);
            assert!(seen.insert(observed));
            assert_eq!(current_thread_id(), owner);
        }
        // Threads created after earlier children exit keep their own identity.
        for _ in 0..8 {
            let child = thread::spawn(current_thread_id);
            let expected = child.thread().id();
            let observed = child.join().unwrap();
            assert_eq!(observed, expected);
            assert!(seen.insert(observed));
        }
    }
}
