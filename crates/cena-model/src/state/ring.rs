//! A bounded, oldest-first buffer that can still be read as one slice.
//!
//! # Why this exists
//!
//! Three buffers drop their oldest entry at a cap -- the stream scrollback
//! ([`MAX_STREAM_LINES`](super::streams::MAX_STREAM_LINES), 2,000), the
//! prompt chunk ([`MAX_CHUNK_LINES`](super::chunks::MAX_CHUNK_LINES), 200) and
//! the message log (200) -- and all three did it with `Vec::remove(0)`, which
//! shifts every remaining element. Found by review: once the main scrollback is
//! full, **every** line routed moved 1,999 others to make room, on the hot path
//! every byte of game text takes.
//!
//! A `VecDeque` makes the drop O(1), but a deque is not a slice, and all three
//! hand callers a `&[T]` -- `GameState::stream`, `Chunk::lines`,
//! `Messages::all` -- that other crates read (`cena-session`'s actor calls
//! `.last()` on a stream). Changing those signatures would ripple for no gain
//! to the caller.
//!
//! So this keeps the deque **contiguous as an invariant**: after every push it
//! is one slice, and [`Self::as_slice`] can hand it out from `&self`. The cost
//! of `make_contiguous` is bounded by reserving a second cap's worth of room
//! the first time the buffer fills, so the ring wraps -- and pays one rotation
//! -- once per `cap` pushes rather than once per push. Amortised O(1).
//!
//! Three users, so the rule of three is met (`plan/05` §−1).

use std::collections::VecDeque;

/// Oldest first, at most `cap` entries, always one contiguous slice.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Ring<T> {
    items: VecDeque<T>,
}

impl<T> Default for Ring<T> {
    fn default() -> Self {
        Self {
            items: VecDeque::new(),
        }
    }
}

impl<T> Ring<T> {
    /// Append, dropping the oldest if `cap` is reached. Returns whether one
    /// was dropped, so the caller can count it (Rule 2.2: nothing is dropped
    /// without saying so).
    pub(crate) fn push(&mut self, item: T, cap: usize) -> bool {
        let dropped = self.items.len() >= cap && self.items.pop_front().is_some();
        // Room for a second cap's worth, once, so the deque wraps once per
        // `cap` pushes rather than on every one.
        if self.items.len() + 1 >= cap && self.items.capacity() < cap.saturating_mul(2) {
            self.items
                .reserve_exact(cap.saturating_mul(2).saturating_sub(self.items.len()));
        }
        self.items.push_back(item);
        self.items.make_contiguous();
        dropped
    }

    /// Everything held, oldest first.
    pub(crate) fn as_slice(&self) -> &[T] {
        // `push` leaves the deque contiguous, and nothing else mutates it
        // without restoring that, so the second half is always empty.
        let (front, back) = self.items.as_slices();
        debug_assert!(back.is_empty(), "Ring lost its contiguity invariant");
        front
    }

    /// Whether nothing is held.
    pub(crate) fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Forget everything.
    pub(crate) fn clear(&mut self) {
        self.items.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::Ring;

    #[test]
    fn it_keeps_the_newest_cap_in_order_and_says_what_it_dropped() {
        let mut ring = Ring::default();
        let mut dropped = 0;
        for i in 0..10_000_usize {
            dropped += usize::from(ring.push(i, 7));
            let held = ring.as_slice().len();
            assert!(held <= 7);
            // Readable as one slice after EVERY push, not only at the end:
            // the wrap point moves, and that is where a contiguity bug hides.
            let expect: Vec<usize> = (i + 1 - held..=i).collect();
            assert_eq!(ring.as_slice(), expect.as_slice(), "after {i}");
        }
        assert_eq!(dropped, 10_000 - 7);
    }

    #[test]
    fn under_the_cap_nothing_is_dropped() {
        let mut ring = Ring::default();
        assert!(!ring.push("a", 3));
        assert!(!ring.push("b", 3));
        assert_eq!(ring.as_slice(), ["a", "b"]);
        ring.clear();
        assert!(ring.is_empty());
    }
}
