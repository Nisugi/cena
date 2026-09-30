//! When the clocks a play window shows next change, so the window asks for
//! a frame then and not before. Moved out of `app.rs` at its cap.

use super::{Duration, Instant, lock};
/// When something in a play window counts down by itself -- roundtime,
/// cast time, a banner, an effect's time left -- how soon it must be drawn
/// again without an event to prompt it: the clocks a quarter of a second,
/// an effect, which counts whole seconds, a second.
pub(super) fn clocks_run(
    snapshot: Option<&cena_session::Snapshot>,
    story: &std::sync::Mutex<crate::story::Story>,
) -> Option<Duration> {
    let state = snapshot.map(|snapshot| &snapshot.state);
    let clocks = state.is_some_and(|state| {
        state.roundtime_remaining().is_some_and(|s| s > 0)
            || state.casttime_remaining().is_some_and(|s| s > 0)
    });
    if clocks || lock(story).alerts_at(Instant::now()).next().is_some() {
        return Some(Duration::from_millis(250));
    }
    let effects = state.is_some_and(|state| {
        state.game_time_now().is_some_and(|now| {
            state
                .effects
                .iter()
                .any(|(id, _)| state.effects.remaining(id, now).is_some_and(|s| s > 0))
                || state
                    .world
                    .pulse
                    .as_ref()
                    .and_then(|pulse| pulse.due(now))
                    .is_some_and(|(_, most)| most > 0)
        })
    });
    effects.then_some(Duration::from_secs(1))
}
