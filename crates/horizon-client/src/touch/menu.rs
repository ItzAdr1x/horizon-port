//! Fingers on the title screen and a world's menus. A finger is the mouse, but its button waits
//! until the gesture is known: a tap clicks where it lifts, and a drag presses where it landed.
//! The game says what lies under each finger (`Here`); this module answers with what a mouse
//! would send (`Input`).

use super::layout::{Button, Layout};
use super::{AWAY, DOUBLE_TAP, Finger, Input, Mode, Role, Touch};
use std::time::{Duration, Instant};

/// A menu finger held still this long is the right button. Longer than play's hold, which
/// splits a stack under a finger resting to read its tooltip: half a second, iOS's own long
/// press.
pub(super) const MENU_HOLD: Duration = Duration::from_millis(500);
/// With a stack in hand, a finger held still on a slot this long puts one item there. No
/// tooltip shows while carrying (`draw_slot_tooltip`), so only a slow tap is at stake.
pub(super) const PAINT_HOLD: Duration = Duration::from_millis(250);
/// Still resting on that slot, the next item comes this long after the first, so a finger
/// sliding across slots gives each one item...
pub(super) const PAINT_DWELL: Duration = Duration::from_millis(450);
/// ...and then one more every this long.
pub(super) const PAINT_REPEAT: Duration = Duration::from_millis(200);

/// What the game shows under a finger, told afresh with every touch event in a menu. The
/// landing one decides the gesture; later ones say where a drag is.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Here {
    pub spot: Spot,
    /// Inside a list that scrolls: the physical pixels one wheel line moves it.
    pub list: Option<f32>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Spot {
    /// Buttons, rows, labels, the background: a tap clicks, a drag is the mouse's drag.
    #[default]
    Plain,
    Slot(SlotSpot),
    /// Beyond the container's panel (slot -999): a click there throws what is carried.
    Outside,
    /// A slider's track: inside a list only a sideways drag moves it.
    Slider,
    /// A scrollbar: a drag holds it, even inside its list.
    Grab,
    /// A text field: a tap focuses it and asks for the keyboard.
    Field,
    /// A button whose right click means something (a recipe with alternatives).
    Secondary,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SlotSpot {
    /// The game's slot id, opaque here: the same slot always answers the same id.
    pub id: u32,
    pub filled: bool,
    pub kind: SlotKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlotKind {
    Normal,
    /// A result or output (the crafting grid's, a station's, a furnace's): a take crafts, trades
    /// or smelts, nothing is put there, and Shift on the crafting grid's crafts all it can.
    Result,
    /// A creative item tab's slot: clicks make copies.
    Display,
    /// Creative's destroy slot: with Shift it empties the whole inventory.
    Trash,
}

/// The lib's own mouse buttons; the bin maps them to winit's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Right,
    /// The pick block button: a full copy in creative (CLONE).
    Middle,
}

/// Finger `finger` puts one item from the hand into the slot at `at` (physical pixels): the
/// mouse's right click there, which the game sends only if its menu says the click moves
/// exactly that one item. The game answers at once with `Touch::placed`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlaceOne {
    pub finger: u64,
    pub at: [f64; 2],
}

/// What the game did with a `PlaceOne`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlaceResult {
    /// The item went in, and the hand holds more.
    Placed,
    /// The slot takes none of it (another item, a full stack, a slot that refuses it): nothing
    /// was clicked.
    Blocked,
    /// No more: the hand ran out, the screen changed, or the click did something else.
    Ended,
}

/// What a finger on a menu is doing, from landing to lifting.
#[derive(Clone, Debug)]
pub(super) enum Gesture {
    /// Down and hovering where it landed; nothing is pressed until the gesture is known.
    Pending {
        landed: Instant,
        from: [f64; 2],
        here: Here,
        /// Held past `MENU_HOLD` on something a hold doesn't change: still a tap.
        held: bool,
    },
    /// A mouse button is down: pressed where the finger landed, the pointer following it.
    Pressing {
        press: Press,
        /// Shift went down with the press and goes up with its release.
        shifted: bool,
        /// Beyond the panel at this point, where lifting throws what is carried: the hint.
        outside: Option<[f64; 2]>,
    },
    /// A list scrolls under the finger: wheel lines where it landed.
    Scrolling {
        from: [f64; 2],
        last: f64,
        /// Pixels dragged that don't make a whole line yet.
        rest: f64,
        line: f32,
    },
    /// A stack in hand put down one item at a time (`Input::PlaceOne`). No button is down
    /// between items, so a lift, a cancel or a scene change has nothing to undo, and it never
    /// becomes a drag.
    Painting(Paint),
    /// A long press clicked already, or did all it will: lifting only hovers away.
    Done,
    /// On the Shift button.
    Shift,
}

impl Gesture {
    /// The finger is the mouse pointer: anything but the Shift button.
    fn points(&self) -> bool {
        !matches!(self, Gesture::Shift)
    }
}

/// A painting finger: the first item where it held still, one on each slot it slides onto for
/// the first time, more while it rests on one.
#[derive(Clone, Debug)]
pub(super) struct Paint {
    /// Where the finger is: the ring follows it.
    at: [f64; 2],
    aim: Aim,
    /// The slots this stroke reached, and whether each took an item. One that did takes the
    /// next only after a dwell; one that took none is passed over.
    visited: Vec<(u32, bool)>,
}

/// The slot a painting finger puts items on, at `point`: inside it, where the finger entered,
/// so a fingertip rolling into a gap still clicks the slot.
#[derive(Clone, Copy, Debug)]
enum Aim {
    /// Nothing to paint: beyond the panel, or a slot's pitch of background from the last slot.
    Off,
    /// A `PlaceOne` went out at `sent`. If its item lands, the next waits `then`.
    Asked {
        slot: u32,
        point: [f64; 2],
        sent: Instant,
        then: Duration,
    },
    /// The next item goes at `next`; the ring fills from `since`.
    Due {
        slot: u32,
        point: [f64; 2],
        since: Instant,
        next: Instant,
    },
    /// The slot takes none: no ring, no click.
    Refused { slot: u32 },
}

impl Aim {
    fn slot(self) -> Option<u32> {
        match self {
            Aim::Off => None,
            Aim::Asked { slot, .. } | Aim::Due { slot, .. } | Aim::Refused { slot } => Some(slot),
        }
    }
}

impl Paint {
    /// Finger `finger` held still on `slot`, at `point`: the first item, now.
    fn start(finger: u64, slot: u32, point: [f64; 2], now: Instant, out: &mut Vec<Input>) -> Self {
        let mut paint = Paint {
            at: point,
            aim: Aim::Off,
            visited: Vec::new(),
        };
        paint.ask(finger, slot, point, now, PAINT_DWELL, out);
        paint
    }

    fn ask(
        &mut self,
        finger: u64,
        slot: u32,
        point: [f64; 2],
        now: Instant,
        then: Duration,
        out: &mut Vec<Input>,
    ) {
        out.push(Input::PlaceOne(PlaceOne { finger, at: point }));
        self.aim = Aim::Asked {
            slot,
            point,
            sent: now,
            then,
        };
    }

    /// The finger moved to `at`, over `here`. Another slot is aimed at once: a new one gets an
    /// item, a visited one waits for the dwell. A gap keeps the slot.
    fn slide(
        &mut self,
        finger: u64,
        at: [f64; 2],
        here: &Here,
        layout: &Layout,
        now: Instant,
        out: &mut Vec<Input>,
    ) {
        self.at = at;
        if let Aim::Asked { .. } = self.aim {
            return;
        }
        match here.spot {
            Spot::Slot(s) if Some(s.id) == self.aim.slot() => {}
            Spot::Slot(s) if s.kind != SlotKind::Normal => self.aim = Aim::Refused { slot: s.id },
            Spot::Slot(s) => match self.visited.iter().find(|(slot, _)| *slot == s.id) {
                None => self.ask(finger, s.id, at, now, PAINT_DWELL, out),
                Some((_, true)) => {
                    self.aim = Aim::Due {
                        slot: s.id,
                        point: at,
                        since: now,
                        next: now + PAINT_DWELL,
                    }
                }
                Some((_, false)) => self.aim = Aim::Refused { slot: s.id },
            },
            Spot::Outside => self.aim = Aim::Off,
            _ => {
                let point = match self.aim {
                    Aim::Due { point, .. } => point,
                    _ => return,
                };
                let away = ((at[0] - point[0]) as f32).hypot((at[1] - point[1]) as f32);
                if away > SLOT_PITCH * layout.gui {
                    self.aim = Aim::Off;
                }
            }
        }
    }

    /// Once a frame: the next item, if its time has come. A late frame sends one, never a
    /// burst to catch up.
    fn due(&mut self, finger: u64, now: Instant, out: &mut Vec<Input>) {
        if let Aim::Due {
            slot, point, next, ..
        } = self.aim
            && now >= next
        {
            self.ask(finger, slot, point, now, PAINT_REPEAT, out);
        }
    }

    /// The game's answer to the `PlaceOne` out. False: the painting is over.
    fn placed(&mut self, result: PlaceResult) -> bool {
        let Aim::Asked {
            slot,
            point,
            sent,
            then,
        } = self.aim
        else {
            return true;
        };
        let took = match result {
            PlaceResult::Placed => true,
            PlaceResult::Blocked => false,
            PlaceResult::Ended => return false,
        };
        match self.visited.iter_mut().find(|(s, _)| *s == slot) {
            Some(visit) => visit.1 = took,
            None => self.visited.push((slot, took)),
        }
        self.aim = if took {
            Aim::Due {
                slot,
                point,
                since: sent,
                next: sent + then,
            }
        } else {
            Aim::Refused { slot }
        };
        true
    }

    /// The ring toward the next item: around the finger, `held` of `wait` filled. None while
    /// no item is to come.
    pub(super) fn ring(&self, now: Instant) -> Option<([f64; 2], Duration, Duration)> {
        match self.aim {
            Aim::Due { since, next, .. } => {
                Some((self.at, now.duration_since(since), next - since))
            }
            _ => None,
        }
    }
}

/// What a menu finger's button is down for.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Press {
    /// The mouse's own drag: a slider, a scrollbar, a button pressed and slid off.
    Plain,
    /// A stack picked up with an empty hand from `origin`, where the finger landed at `from`:
    /// it is put down where the finger lifts.
    Carry { origin: SlotSpot, from: [f64; 2] },
    /// The carried stack, spread evenly over the slots the finger crosses (QUICK_CRAFT 0).
    Spread,
    /// The right button on a stack, under an empty hand: half of it taken (PICKUP 1).
    Split,
}

impl Press {
    fn button(self) -> MouseButton {
        match self {
            Press::Split => MouseButton::Right,
            Press::Plain | Press::Carry { .. } | Press::Spread => MouseButton::Left,
        }
    }
    /// Lifting it beyond the panel throws a stack.
    fn throws(self) -> bool {
        !matches!(self, Press::Plain | Press::Split)
    }
}

/// What holding a pending finger still for `hold_time` does: the mouse's right button, one rule
/// everywhere, but a stack in hand goes on a slot one item at a time.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Hold {
    /// The button goes down, and comes up where the finger lifts.
    Press(Press),
    /// A click at once.
    Click(MouseButton),
    /// One item from the hand on this slot, then more (`Gesture::Painting`).
    Paint(u32),
    /// Nothing, and no tap at the lift either: a stack in hand held on a slot nothing goes on,
    /// where a click would craft, copy or destroy.
    Done,
    /// Nothing: the finger keeps hovering, and still taps when it lifts.
    Nothing,
}

/// What a hold on where the finger landed does, `carrying` a stack or not. Shift held or
/// latched keeps a carried finger's shifted tap: it paints nothing.
pub(super) fn hold(landed: &Here, carrying: bool, shifted: bool) -> Hold {
    match landed.spot {
        Spot::Slot(s) if carrying => match s.kind {
            SlotKind::Normal if shifted => Hold::Nothing,
            SlotKind::Normal => Hold::Paint(s.id),
            SlotKind::Result | SlotKind::Display | SlotKind::Trash => Hold::Done,
        },
        // Beyond the panel, one item is dropped (slot -999).
        Spot::Outside if carrying => Hold::Click(MouseButton::Right),
        Spot::Slot(s) if s.filled => match s.kind {
            SlotKind::Normal => Hold::Press(Press::Split),
            // Vanilla's right click on the result takes one craft, as a left one does.
            SlotKind::Result => Hold::Click(MouseButton::Right),
            // The pick block button: a full stack of the copy.
            SlotKind::Display => Hold::Click(MouseButton::Middle),
            SlotKind::Trash => Hold::Nothing,
        },
        Spot::Secondary => Hold::Click(MouseButton::Right),
        _ => Hold::Nothing,
    }
}

/// How long a finger holds still before its hold acts: `PAINT_HOLD` for a stack in hand on a
/// slot, `MENU_HOLD` elsewhere.
pub(super) fn hold_time(landed: &Here, carrying: bool) -> Duration {
    if carrying && matches!(landed.spot, Spot::Slot(_)) {
        PAINT_HOLD
    } else {
        MENU_HOLD
    }
}

/// A slot's pitch in GUI pixels: its 16-pixel box and the gap to the next.
const SLOT_PITCH: f32 = 18.0;

/// The finger has left where it landed: its slot, for a slot outside a list (a tap may wander
/// inside a slot); its slop anywhere else. With a stack in hand, a resting fingertip rolls into
/// the 2-pixel gaps beside its slot, and a spread there would scatter the stack: it leaves only
/// for another slot, beyond the panel, or a slot's pitch of the panel's background away.
pub(super) fn left(
    landed: &Here,
    from: [f64; 2],
    here: &Here,
    at: [f64; 2],
    layout: &Layout,
    carrying: bool,
) -> bool {
    let moved = ((at[0] - from[0]) as f32).hypot((at[1] - from[1]) as f32);
    match (landed.spot, landed.list) {
        (Spot::Slot(s), None) => match here.spot {
            Spot::Slot(t) => t.id != s.id,
            Spot::Outside => true,
            _ => !carrying || moved > SLOT_PITCH * layout.gui,
        },
        _ => moved > layout.slop(),
    }
}

/// What a finger becomes once it leaves where it landed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Drag {
    /// The mouse button goes down where it landed, for this.
    Press(Press),
    /// The list's wheel turns, a line every this many physical pixels.
    Scroll(f32),
}

/// What a finger that landed at `from` and is now at `at` becomes. In a list it scrolls, but a
/// scrollbar is held, and a slider or field moved sideways is the mouse's drag.
pub(super) fn drag(
    landed: &Here,
    carrying: bool,
    from: [f64; 2],
    at: [f64; 2],
    shifted: bool,
) -> Drag {
    let sideways = (at[0] - from[0]).abs() > (at[1] - from[1]).abs();
    Drag::Press(match (landed.list, landed.spot) {
        (_, Spot::Grab) => Press::Plain,
        (Some(_), Spot::Slider | Spot::Field) if sideways => Press::Plain,
        (Some(line), _) => return Drag::Scroll(line),
        _ if carrying => Press::Spread,
        // Shifted, the press quick-moves the stack away: nothing is left in hand to put down.
        _ if shifted => Press::Plain,
        // A creative item's copy has nowhere to go back to.
        (None, Spot::Slot(origin))
            if origin.filled && matches!(origin.kind, SlotKind::Normal | SlotKind::Result) =>
        {
            Press::Carry { origin, from }
        }
        _ => Press::Plain,
    })
}

/// Whole wheel lines in `dy` more pixels dragged, keeping what is left over: up (positive), the
/// list's top coming into view, when the finger goes down.
pub(super) fn wheel_lines(rest: &mut f64, dy: f64, line: f32) -> i32 {
    *rest += dy;
    let lines = (*rest / f64::from(line)).trunc();
    *rest -= lines * f64::from(line);
    lines as i32
}

/// A press and release of `button` where the pointer is.
fn click(out: &mut Vec<Input>, button: MouseButton, fresh: bool) {
    out.push(Input::Press { button, fresh });
    out.push(Input::Release(button));
}

/// The Shift button as the iPhone keyboard's shift key: a tap shifts the next click, a quick
/// second tap locks it, a tap unlocks, and holding it shifts every click until it lifts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Latch {
    #[default]
    Off,
    Once,
    Locked,
}

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct ShiftKey {
    pub(super) latch: Latch,
    tapped: Option<Instant>,
    held: bool,
}

impl ShiftKey {
    /// A finger lands on the button: Off, then Once, then (quickly) Locked; a slow second tap,
    /// or a tap while locked, lets go.
    fn tap(&mut self, now: Instant) {
        let quick = self
            .tapped
            .is_some_and(|t| now.duration_since(t) <= DOUBLE_TAP);
        self.latch = match self.latch {
            Latch::Off => Latch::Once,
            Latch::Once if quick => Latch::Locked,
            Latch::Once | Latch::Locked => Latch::Off,
        };
        self.tapped = Some(now);
        self.held = true;
    }
    fn lift(&mut self) {
        self.held = false;
    }
    pub(super) fn active(&self) -> bool {
        self.held || self.latch != Latch::Off
    }
    /// Whether a click would be shifted. `latched`: the latch may shift it; creative's trash
    /// slot, where Shift empties the whole inventory, is shifted only by a held button.
    fn shifts(&self, latched: bool) -> bool {
        if latched { self.active() } else { self.held }
    }
    /// Whether a click is shifted, using Once up.
    fn take(&mut self, latched: bool) -> bool {
        let shifted = self.shifts(latched);
        if latched && self.latch == Latch::Once {
            self.latch = Latch::Off;
        }
        shifted
    }
}

impl Touch {
    /// A finger lands on the title screen or a menu.
    pub(super) fn land_menu(
        &mut self,
        layout: &Layout,
        at: [f64; 2],
        here: Here,
        now: Instant,
        out: &mut Vec<Input>,
    ) -> Role {
        let px = [at[0] as f32, at[1] as f32];
        if self.context.mode == Mode::Menu && layout.on(Button::Back, px) {
            out.push(Input::Back);
            return Role::Spent;
        }
        if self.context.slots && layout.on(Button::Shift, px) {
            self.menus.shift.tap(now);
            return Role::Menu(Gesture::Shift);
        }
        // A desk has one mouse.
        let pointing = |f: &Finger| matches!(&f.role, Role::Menu(g) if g.points());
        if self.fingers.iter().any(pointing) {
            return Role::Spent;
        }
        out.push(Input::Pointer(at));
        Role::Menu(Gesture::Pending {
            landed: now,
            from: at,
            here,
            held: false,
        })
    }

    /// Menu finger `finger` moves to `at`, over `here`.
    pub(super) fn slide_menu(
        &mut self,
        layout: &Layout,
        finger: u64,
        gesture: &mut Gesture,
        at: [f64; 2],
        here: Here,
        now: Instant,
        out: &mut Vec<Input>,
    ) {
        if let Gesture::Pending {
            from, here: landed, ..
        } = *gesture
        {
            if !left(&landed, from, &here, at, layout, self.context.carrying) {
                out.push(Input::Pointer(at));
                return;
            }
            let latched = !trash(&landed);
            let carrying = self.context.carrying;
            match drag(
                &landed,
                carrying,
                from,
                at,
                self.menus.shift.shifts(latched),
            ) {
                Drag::Press(press) => {
                    let shifted = self.menus.shift.take(latched);
                    out.push(Input::Pointer(from));
                    press_down(out, press.button(), shifted, true);
                    *gesture = Gesture::Pressing {
                        press,
                        shifted,
                        outside: None,
                    };
                }
                Drag::Scroll(line) => {
                    // Nothing hovers while a list moves.
                    out.push(Input::Pointer(AWAY));
                    *gesture = Gesture::Scrolling {
                        from,
                        last: from[1],
                        rest: 0.0,
                        line,
                    };
                }
            }
        }
        match gesture {
            Gesture::Shift => return,
            Gesture::Scrolling {
                from,
                last,
                rest,
                line,
            } => {
                let lines = wheel_lines(rest, at[1] - *last, *line);
                *last = at[1];
                if lines != 0 {
                    // The list under where the finger landed turns, one line at a time, as a
                    // stepped list (a merchant's offers, creative's rows) takes them.
                    out.push(Input::Pointer(*from));
                    let step = Input::Wheel(lines.signum() as f32);
                    out.extend(std::iter::repeat_n(step, lines.unsigned_abs() as usize));
                    out.push(Input::Pointer(AWAY));
                }
                return;
            }
            Gesture::Pressing { press, outside, .. } => {
                *outside = (press.throws() && here.spot == Spot::Outside).then_some(at);
            }
            Gesture::Painting(_) if self.paint_stops() => *gesture = Gesture::Done,
            Gesture::Painting(paint) => paint.slide(finger, at, &here, layout, now, out),
            _ => {}
        }
        out.push(Input::Pointer(at));
    }

    /// A painting finger puts no more down: the hand ran out, or Shift went down, which would
    /// make the next click a quick move.
    fn paint_stops(&self) -> bool {
        !self.context.carrying || self.menus.shift.active()
    }

    /// The game's answer to finger `finger`'s `PlaceOne`, given before the next event or frame.
    /// An answer to no question does nothing.
    pub fn placed(&mut self, finger: u64, result: PlaceResult) {
        let Some(f) = self.fingers.iter_mut().find(|f| f.id == finger) else {
            return;
        };
        if let Role::Menu(gesture) = &mut f.role
            && let Gesture::Painting(paint) = gesture
            && !paint.placed(result)
        {
            *gesture = Gesture::Done;
        }
    }

    /// Whether a click on `here` is shifted, as the Shift button stands.
    fn shift_for(&mut self, here: &Here) -> bool {
        self.menus.shift.take(!trash(here))
    }

    /// A menu finger lifts at `at`, over `here`.
    pub(super) fn lift_menu(
        &mut self,
        gesture: Gesture,
        at: [f64; 2],
        here: Here,
        out: &mut Vec<Input>,
    ) {
        if let Gesture::Shift = gesture {
            self.menus.shift.lift();
            return;
        }
        self.menus.rest = Some(at);
        if let Gesture::Scrolling { .. } = gesture {
            out.push(Input::Pointer(AWAY));
            return;
        }
        let (at, here) = match gesture {
            // A stack in hand put down from a slot's gap goes on the slot the finger landed on,
            // which it never left (`left`).
            Gesture::Pending {
                from, here: landed, ..
            } if self.context.carrying
                && matches!((landed.spot, landed.list), (Spot::Slot(_), None))
                && !matches!(here.spot, Spot::Slot(_) | Spot::Outside) =>
            {
                (from, landed)
            }
            _ => (at, here),
        };
        out.push(Input::Pointer(at));
        match gesture {
            // Fingers miss on a big screen, and a stack thrown into the world can't be taken
            // back: throwing is a drag beyond the panel, never a tap.
            Gesture::Pending { .. } if self.context.carrying && here.spot == Spot::Outside => {}
            // A tap clicks where it lifts. Two taps on one slot reach the game as two clicks at
            // their own lifts, so its double click (PICKUP_ALL) is the game's to decide, on its
            // own clock.
            Gesture::Pending { .. } => {
                let shifted = self.shift_for(&here);
                press_down(out, MouseButton::Left, shifted, false);
                release(out, MouseButton::Left, shifted);
                if here.spot == Spot::Field {
                    out.push(Input::Keyboard);
                }
            }
            Gesture::Pressing {
                press: Press::Carry { origin, from },
                ..
            } => {
                out.push(Input::Release(MouseButton::Left));
                // The stack put down where the finger lifts, and whatever that leaves in hand put
                // back where it came from (a stack it was swapped with, a remainder). Nothing is
                // put down on a result: a click there would craft again, so a crafted stack goes
                // back nowhere, and one lifted over a result stays in hand or goes back.
                let back = origin.kind == SlotKind::Normal;
                match here.spot {
                    Spot::Slot(b) if b.id == origin.id => {}
                    Spot::Outside => click(out, MouseButton::Left, true),
                    Spot::Slot(b) => {
                        if b.kind != SlotKind::Result {
                            out.push(Input::PutDown(at));
                        }
                        if back {
                            out.push(Input::PutDown(from));
                        }
                    }
                    _ if back => out.push(Input::PutDown(from)),
                    _ => {}
                }
            }
            Gesture::Pressing { press, shifted, .. } => release(out, press.button(), shifted),
            // Every item already landed: the lift only hovers away.
            Gesture::Painting(_) | Gesture::Scrolling { .. } | Gesture::Done | Gesture::Shift => {}
        }
        out.push(Input::Pointer(AWAY));
    }

    /// Once a frame: the menu finger held still long enough does what a hold does, and a
    /// painting finger puts its next item down when that is due.
    pub(super) fn menu_holds(&mut self, now: Instant, out: &mut Vec<Input>) {
        let stops = self.paint_stops();
        let (carrying, shifted) = (self.context.carrying, self.menus.shift.active());
        for finger in &mut self.fingers {
            let Role::Menu(gesture) = &mut finger.role else {
                continue;
            };
            if let Gesture::Painting(paint) = gesture {
                if stops {
                    *gesture = Gesture::Done;
                } else {
                    paint.due(finger.id, now, out);
                }
                continue;
            }
            let Gesture::Pending {
                landed,
                from,
                here,
                held: held @ false,
            } = gesture
            else {
                continue;
            };
            if now.duration_since(*landed) < hold_time(here, carrying) {
                continue;
            }
            let (from, here) = (*from, *here);
            match hold(&here, carrying, shifted) {
                Hold::Paint(slot) => {
                    *gesture = Gesture::Painting(Paint::start(finger.id, slot, from, now, out));
                }
                Hold::Press(press) => {
                    let shifted = self.menus.shift.take(true);
                    out.push(Input::Pointer(from));
                    press_down(out, press.button(), shifted, true);
                    let outside = press.throws() && here.spot == Spot::Outside;
                    *gesture = Gesture::Pressing {
                        press,
                        shifted,
                        outside: outside.then_some(from),
                    };
                }
                Hold::Click(button) => {
                    out.push(Input::Pointer(from));
                    click(out, button, true);
                    *gesture = Gesture::Done;
                }
                Hold::Done => *gesture = Gesture::Done,
                Hold::Nothing => *held = true,
            }
        }
    }

    /// Let go of what a menu finger holds without acting: a cancelled touch, or a scene that
    /// changed under it. Its button comes up where the finger was, clicking nothing, so a slider
    /// keeps what the drag gave it; then nothing hovers.
    pub(super) fn let_go(&mut self, gesture: Gesture, out: &mut Vec<Input>) {
        match gesture {
            Gesture::Shift => {
                self.menus.shift.lift();
                return;
            }
            Gesture::Pressing { press, shifted, .. } => {
                out.push(Input::Abort(press.button()));
                if shifted {
                    out.push(Input::Shift(false));
                }
            }
            _ => {}
        }
        out.push(Input::Pointer(AWAY));
    }

    /// The physical pixels tooltips keep clear above the pointer: a fingertip while a finger is
    /// on a menu, none for a mouse.
    pub fn tooltip_lift(&self, layout: &Layout) -> f32 {
        let pointing = self
            .fingers
            .iter()
            .any(|f| matches!(&f.role, Role::Menu(g) if g.points()));
        if pointing {
            layout.finger_clearance()
        } else {
            0.0
        }
    }

    /// Where a carried stack is drawn while nothing hovers: where the last finger lifted.
    pub fn rest(&self) -> Option<[f64; 2]> {
        self.menus.rest
    }
}

/// Creative's trash slot, which the Shift latch never reaches.
fn trash(here: &Here) -> bool {
    matches!(here.spot, Spot::Slot(s) if s.kind == SlotKind::Trash)
}

/// `button` goes down, Shift first when the press is shifted.
fn press_down(out: &mut Vec<Input>, button: MouseButton, shifted: bool, fresh: bool) {
    if shifted {
        out.push(Input::Shift(true));
    }
    out.push(Input::Press { button, fresh });
}

/// `button` comes up, then Shift if it went down with it.
fn release(out: &mut Vec<Input>, button: MouseButton, shifted: bool) {
    out.push(Input::Release(button));
    if shifted {
        out.push(Input::Shift(false));
    }
}

/// What menus keep between fingers.
#[derive(Debug, Default)]
pub(super) struct Menus {
    pub(super) shift: ShiftKey,
    /// Where the last menu finger lifted.
    rest: Option<[f64; 2]>,
}

#[cfg(test)]
mod tests;
