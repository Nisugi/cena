//! `attribute` terminates on every input, including the degenerate one.
//!
//! Split out of `text.rs` under Rule 4.1 -- move code down, do not raise the
//! cap -- when the empty-name regression test pushed that file to 410 of its
//! 400 lines. The arch suite caught it in the same run that added it, which is
//! the ratchet doing its job.

#[cfg(test)]
mod tests {
    /// An empty attribute name terminates, and answers "absent".
    ///
    /// # Why this runs on a thread with a deadline
    ///
    /// The defect was an INFINITE LOOP. A test that simply calls the function
    /// and asserts on its result cannot fail -- if the bug is present the test
    /// never returns, and the whole suite hangs with no failing assertion and
    /// no name to point at. So the call goes to a worker and the assertion is
    /// on whether it came back.
    ///
    /// Verified both ways: with the `name.is_empty()` guard removed, this test
    /// reports the timeout rather than hanging the run.
    #[test]
    fn an_empty_attribute_name_terminates() {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(crate::text::attribute("<nav rm='1'/>", ""));
        });

        // `Timeout` by name, not `Err(_)`: `Disconnected` would mean the
        // worker panicked, which is a different failure and must not be
        // reported as a hang.
        match rx.recv_timeout(std::time::Duration::from_secs(5)) {
            Ok(answer) => assert_eq!(
                answer, None,
                "an empty name matches no attribute, so `None` is the honest                  answer -- there is no attribute with no name"
            ),
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                panic!("the worker thread died without answering")
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => panic!(
                "`attribute(tag, \"\")` did not return within 5s.                  `find(\"\")` yields Some(0) at every offset, so `from` never                  advances and the loop is infinite. This is a `pub fn` in a                  `pub mod`."
            ),
        }
    }
}
