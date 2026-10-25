//! The key mappings the touch controls hold, as the game has them, and the presses and
//! releases still to send: every state a mapping takes at the game lasts long enough for a game
//! tick to see it.

use super::Input;
use std::time::{Duration, Instant};

/// How long a mapping stays down, and then up, at the game before its next edge goes out. Ticks
/// come every 50 ms, in some frame; a quicker press, or the gap between the two taps of a quick
/// double tap (flying), could fall between two of them.
pub(super) const MIN_STATE: Duration = Duration::from_millis(70);

#[derive(Debug)]
struct Mapping {
    name: &'static str,
    /// Down at the game.
    at_game: bool,
    /// When the game got its last edge.
    since: Instant,
    /// Edges still to send. They alternate, so each flips the state the game will have.
    pending: u32,
}

impl Mapping {
    fn wanted(&self) -> bool {
        self.at_game ^ (self.pending % 2 == 1)
    }
    fn ready(&self, now: Instant) -> bool {
        now >= self.since + MIN_STATE
    }
    fn send(&mut self, now: Instant, out: &mut Vec<Input>) {
        self.at_game = !self.at_game;
        self.since = now;
        out.push(Input::Mapping(self.name, self.at_game));
    }
}

#[derive(Debug, Default)]
pub(super) struct Keys(Vec<Mapping>);

impl Keys {
    /// Down at the game, or going to be once the edges still to send have gone out.
    pub(super) fn held(&self, name: &str) -> bool {
        self.0
            .iter()
            .any(|m| m.name == name && (m.at_game || m.wanted()))
    }

    /// Press or release `name`: now, or once the game has had its current state long enough.
    pub(super) fn set(
        &mut self,
        name: &'static str,
        down: bool,
        now: Instant,
        out: &mut Vec<Input>,
    ) {
        let i = match self.0.iter().position(|m| m.name == name) {
            Some(i) => i,
            None if !down => return,
            None => {
                self.0.push(Mapping {
                    name,
                    at_game: false,
                    since: now,
                    pending: 0,
                });
                let m = self.0.last_mut().unwrap();
                m.send(now, out);
                return;
            }
        };
        let m = &mut self.0[i];
        if m.wanted() == down {
            return;
        }
        if m.pending == 0 && m.ready(now) {
            m.send(now, out);
        } else {
            m.pending += 1;
        }
    }

    /// Once a frame: one edge for each mapping whose current state the game has had long enough.
    pub(super) fn due(&mut self, now: Instant, out: &mut Vec<Input>) {
        for m in &mut self.0 {
            if m.pending > 0 && m.ready(now) {
                m.pending -= 1;
                m.send(now, out);
            }
        }
    }

    /// A screen opened: what the game has down goes up at once, and nothing waiting is sent.
    pub(super) fn release_all(&mut self, now: Instant, out: &mut Vec<Input>) {
        for m in &mut self.0 {
            m.pending = 0;
            if m.at_game {
                m.send(now, out);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const JUMP: &str = "key.jump";

    fn at(now: Instant, ms: u64) -> Instant {
        now + Duration::from_millis(ms)
    }

    #[test]
    fn a_press_goes_out_at_once_and_its_release_after_min_state() {
        let (mut keys, now, mut out) = (Keys::default(), Instant::now(), Vec::new());
        keys.set(JUMP, true, now, &mut out);
        keys.set(JUMP, false, at(now, 20), &mut out);
        assert_eq!(out, [Input::Mapping(JUMP, true)]);
        keys.due(at(now, 69), &mut out);
        keys.due(at(now, 70), &mut out);
        assert_eq!(
            out,
            [Input::Mapping(JUMP, true), Input::Mapping(JUMP, false)]
        );
        assert!(!keys.held(JUMP));
    }

    #[test]
    fn a_double_tap_keeps_both_presses_spaced_from_their_delivery() {
        let (mut keys, now, mut out) = (Keys::default(), Instant::now(), Vec::new());
        keys.set(JUMP, true, now, &mut out);
        keys.set(JUMP, false, at(now, 40), &mut out);
        keys.set(JUMP, true, at(now, 55), &mut out);
        keys.set(JUMP, false, at(now, 60), &mut out);
        // A stalled frame: one edge goes out, and the next waits MIN_STATE from it.
        keys.due(at(now, 150), &mut out);
        keys.due(at(now, 200), &mut out);
        keys.due(at(now, 220), &mut out);
        keys.due(at(now, 289), &mut out);
        keys.due(at(now, 290), &mut out);
        assert_eq!(
            out,
            [
                Input::Mapping(JUMP, true),
                Input::Mapping(JUMP, false),
                Input::Mapping(JUMP, true),
                Input::Mapping(JUMP, false),
            ]
        );
    }

    #[test]
    fn a_screen_releases_what_the_game_has_down_even_with_its_release_waiting() {
        let (mut keys, now, mut out) = (Keys::default(), Instant::now(), Vec::new());
        keys.set(JUMP, true, now, &mut out);
        keys.set(JUMP, false, at(now, 30), &mut out);
        assert!(keys.held(JUMP));
        keys.release_all(at(now, 50), &mut out);
        keys.due(at(now, 200), &mut out);
        assert_eq!(
            out,
            [Input::Mapping(JUMP, true), Input::Mapping(JUMP, false)]
        );
        assert!(!keys.held(JUMP));
    }

    #[test]
    fn a_press_waiting_behind_a_release_is_dropped_by_a_screen() {
        let (mut keys, now, mut out) = (Keys::default(), Instant::now(), Vec::new());
        keys.set(JUMP, true, now, &mut out);
        keys.due(at(now, 100), &mut out);
        keys.set(JUMP, false, at(now, 100), &mut out);
        keys.set(JUMP, true, at(now, 110), &mut out);
        keys.release_all(at(now, 120), &mut out);
        keys.due(at(now, 300), &mut out);
        assert_eq!(
            out,
            [Input::Mapping(JUMP, true), Input::Mapping(JUMP, false)]
        );
    }

    #[test]
    fn asking_for_the_state_already_wanted_does_nothing() {
        let (mut keys, now, mut out) = (Keys::default(), Instant::now(), Vec::new());
        keys.set(JUMP, false, now, &mut out);
        keys.set(JUMP, true, now, &mut out);
        keys.set(JUMP, true, at(now, 10), &mut out);
        assert_eq!(out, [Input::Mapping(JUMP, true)]);
    }
}
