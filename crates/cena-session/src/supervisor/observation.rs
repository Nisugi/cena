//! Keep the read seam available while a connector or retry timer is waiting
//! -- and the command inbox answered, so nothing sent during an outage waits
//! for the outage to end (`plan/12` §5.1; review finding 6).

use super::{ConnectError, Connector, Generation, SupervisedSession};
use std::time::Duration;

impl<C: Connector> SupervisedSession<C> {
    pub(super) async fn connect_observed(
        &mut self,
        generation: Generation,
    ) -> Option<Result<C::Source, ConnectError>> {
        let connecting = self.connector.connect(generation);
        tokio::pin!(connecting);
        loop {
            tokio::select! {
                biased;
                () = self.core.cancel.cancelled() => return None,
                connected = &mut connecting => return Some(connected),
                Some(request) = self.core.observations.requests.recv() => {
                    self.core.events.answer(request, &self.core.state, self.core.lifecycle);
                }
                Some(message) = self.core.commands.recv() => {
                    self.core.refuse_while_disconnected(message);
                }
            }
        }
    }

    pub(super) async fn wait_observed(&mut self, delay: Duration) -> bool {
        let waiting = tokio::time::sleep(delay);
        tokio::pin!(waiting);
        loop {
            tokio::select! {
                biased;
                () = self.core.cancel.cancelled() => return false,
                () = &mut waiting => return true,
                Some(request) = self.core.observations.requests.recv() => {
                    self.core.events.answer(request, &self.core.state, self.core.lifecycle);
                }
                Some(message) = self.core.commands.recv() => {
                    self.core.refuse_while_disconnected(message);
                }
            }
        }
    }
}
