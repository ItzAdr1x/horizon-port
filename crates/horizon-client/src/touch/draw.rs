//! The touch controls in Pocket Edition's look: translucent squares with pixel-art icons, one art
//! pixel `Layout::unit` physical pixels wide, drawn into the HUD batch. Key caps and the Fn
//! panel's tiles are written in the game's font instead.

use super::functions::Control;
use super::layout::{Button, Cell, KNOB, Layout, Rect};
use super::menu::{Gesture, Hold, Latch, MENU_HOLD, hold};
use super::{HOLD, Mode, Movement, Role, Touch};
use crate::hud::{HudAtlas, HudBatch, rgba};
use std::time::{Duration, Instant};

type Icon = &'static [&'static str];

const ARROW: Icon = &[
    "....##....",
    "...####...",
    "..######..",
    ".########.",
    "##########",
    "...####...",
    "...####...",
    "...####...",
];
const DIAGONAL: Icon = &[
    "#######...",
    "######....",
    "######....",
    "#######...",
    "###.####..",
    "##...####.",
    "#.....####",
    ".......##.",
];
const JUMP: Icon = &[
    "....##....",
    "...####...",
    "..##..##..",
    ".##....##.",
    "##......##",
    "....##....",
    "...####...",
    "..##..##..",
    ".##....##.",
    "##......##",
];
const SNEAK: Icon = &[
    "##......##",
    ".##....##.",
    "..##..##..",
    "...####...",
    "....##....",
    "..........",
    "##########",
    "##########",
];
const DOTS: Icon = &["##..##..##", "##..##..##"];
/// The iPhone keyboard's shift key: hollow, filled for the next click, underlined when locked.
const SHIFT: Icon = &[
    "....##....",
    "...#..#...",
    "..#....#..",
    ".#......#.",
    "###....###",
    "..#....#..",
    "..#....#..",
    "..######..",
];
const SHIFT_ON: Icon = &[
    "....##....",
    "...####...",
    "..######..",
    ".########.",
    "##########",
    "..######..",
    "..######..",
    "..######..",
];
const SHIFT_LOCKED: Icon = &[
    "....##....",
    "...####...",
    "..######..",
    ".########.",
    "##########",
    "..######..",
    "..######..",
    "..######..",
    "..........",
    "..######..",
];
/// An arrow down to the ground: lifting here throws the stack.
const DROP: Icon = &[
    "...####...",
    "...####...",
    "...####...",
    "##########",
    ".########.",
    "..######..",
    "...####...",
    "....##....",
    "..........",
    "##########",
];
const PAUSE: Icon = &["##..##", "##..##", "##..##", "##..##", "##..##", "##..##"];
/// A speech bubble.
const CHAT: Icon = &[
    ".#########.",
    "###########",
    "##.##.##.##",
    "###########",
    ".#########.",
    "..##.......",
    "..#........",
];

/// Which way an icon points: as drawn, or turned.
#[derive(Clone, Copy)]
enum Turn {
    Up,
    Down,
    Left,
    Right,
    Mirror,
}

const OUTLINE: u32 = 0x9600_0000;
const PLATE: u32 = 0x6e00_0000;
const ICON: u32 = 0xdcff_ffff;
/// Over play while the Fn panel is open.
const SHADE: u32 = 0x8000_0000;
/// The game font's capitals, in GUI pixels: seven rows, and a line nine apart.
const CAPITALS: f32 = 7.0;
const LINE: f32 = 9.0;

fn pressed_plate() -> u32 {
    rgba(255, 255, 255, 120)
}

struct Pen<'a> {
    batch: &'a mut HudBatch,
    atlas: &'a HudAtlas,
    layout: &'a Layout,
}

impl Pen<'_> {
    fn fill(&mut self, r: Rect, color: u32) {
        let g = self.layout.gui;
        self.batch
            .fill(self.atlas, r[0] / g, r[1] / g, r[2] / g, r[3] / g, color);
    }
    /// A square with a one-pixel outline, lit while pressed.
    fn plate(&mut self, r: Rect, pressed: bool) {
        let u = self.layout.unit;
        self.fill(r, OUTLINE);
        let inner = [r[0] + u, r[1] + u, r[2] - 2.0 * u, r[3] - 2.0 * u];
        self.fill(inner, if pressed { pressed_plate() } else { PLATE });
    }
    /// `icon` in the middle of `r`, turned.
    fn icon(&mut self, r: Rect, icon: Icon, turn: Turn) {
        let u = self.layout.unit;
        let (rows, cols) = (icon.len() as f32, icon[0].len() as f32);
        let (w, h) = match turn {
            Turn::Left | Turn::Right => (rows, cols),
            _ => (cols, rows),
        };
        let x0 = r[0] + ((r[2] / u - w) / 2.0).floor() * u;
        let y0 = r[1] + ((r[3] / u - h) / 2.0).floor() * u;
        for (y, line) in icon.iter().enumerate() {
            for (x, c) in line.bytes().enumerate() {
                if c != b'#' {
                    continue;
                }
                let (x, y) = (x as f32, y as f32);
                let (px, py) = match turn {
                    Turn::Up => (x, y),
                    Turn::Down => (x, rows - 1.0 - y),
                    Turn::Left => (y, x),
                    Turn::Right => (rows - 1.0 - y, x),
                    Turn::Mirror => (cols - 1.0 - x, y),
                };
                self.fill([x0 + px * u, y0 + py * u, u, u], ICON);
            }
        }
    }
    fn button(&mut self, r: Rect, pressed: bool, icon: Icon, turn: Turn) {
        self.plate(r, pressed);
        self.icon(r, icon, turn);
    }
    /// `text` in the game's font, with its shadow, centred across `r` with its capitals' top at
    /// `top`, on whole GUI pixels.
    fn text(&mut self, text: &str, r: Rect, top: f32, color: u32) {
        let g = self.layout.gui;
        let width = HudBatch::text_width(self.atlas, text);
        let x = ((r[0] + r[2] / 2.0) / g - width / 2.0).round();
        self.batch
            .text(self.atlas, text, x, (top / g).round(), color, true);
    }
    /// A ring of dots around a finger at `at`, filling as it is held `held` of `hold`.
    fn ring(&mut self, at: [f64; 2], held: Duration, hold: Duration) {
        const DOTS: u32 = 12;
        let u = self.layout.unit;
        let lit = (held.as_secs_f32() / hold.as_secs_f32() * DOTS as f32) as u32;
        for i in 0..DOTS {
            let angle = i as f32 / DOTS as f32 * std::f32::consts::TAU;
            let (x, y) = (
                at[0] as f32 + angle.sin() * 12.0 * u,
                at[1] as f32 - angle.cos() * 12.0 * u,
            );
            let color = if i < lit { ICON } else { PLATE };
            self.fill([x - u, y - u, 2.0 * u, 2.0 * u], color);
        }
    }
    /// A disc of `radius` art pixels around `centre`, one row of pixels at a time.
    fn disc(&mut self, centre: [f32; 2], radius: f32, color: u32) {
        let u = self.layout.unit;
        let r = radius.round() as i32;
        for row in -r..r {
            let y = row as f32 + 0.5;
            let half = (radius * radius - y * y).max(0.0).sqrt().round();
            if half > 0.0 {
                let x = (centre[0] / u).round() * u - half * u;
                let top = (centre[1] / u).round() * u + row as f32 * u;
                self.fill([x, top, 2.0 * half * u, u], color);
            }
        }
    }
}

impl Touch {
    /// The controls, once a finger has touched the screen. `sneaking` lights the D-pad's middle.
    pub fn draw(
        &self,
        batch: &mut HudBatch,
        atlas: &HudAtlas,
        layout: &Layout,
        sneaking: bool,
        now: Instant,
    ) {
        if !self.seen {
            return;
        }
        let mut pen = Pen {
            batch,
            atlas,
            layout,
        };
        match self.context.mode {
            Mode::Title => self.draw_menu(&mut pen, now),
            Mode::Menu => {
                pen.button(layout.button(Button::Back), false, ARROW, Turn::Left);
                if self.context.slots {
                    let shift = self.menus.shift;
                    let icon = match shift.latch {
                        Latch::Locked => SHIFT_LOCKED,
                        _ if shift.active() => SHIFT_ON,
                        _ => SHIFT,
                    };
                    pen.button(layout.button(Button::Shift), shift.active(), icon, Turn::Up);
                }
                self.draw_menu(&mut pen, now);
            }
            Mode::Playing if self.panel => self.draw_panel(&mut pen),
            Mode::Playing => {
                match self.context.movement {
                    Movement::DPad => self.draw_dpad(&mut pen, sneaking),
                    Movement::Joystick => self.draw_joystick(&mut pen),
                }
                let held = |button| {
                    self.fingers
                        .iter()
                        .any(|f| matches!(f.role, Role::Button(b) if b == button))
                };
                pen.button(
                    layout.button(Button::Jump),
                    held(Button::Jump),
                    JUMP,
                    Turn::Up,
                );
                let inventory = layout.button(Button::Inventory);
                pen.button(inventory, held(Button::Inventory), DOTS, Turn::Up);
                pen.button(
                    layout.button(Button::Pause),
                    held(Button::Pause),
                    PAUSE,
                    Turn::Up,
                );
                pen.button(
                    layout.button(Button::Chat),
                    held(Button::Chat),
                    CHAT,
                    Turn::Up,
                );
                self.draw_caps(&mut pen);
                self.draw_holds(&mut pen, now);
            }
        }
    }

    fn draw_dpad(&self, pen: &mut Pen<'_>, sneaking: bool) {
        let under = self.fingers.iter().find_map(|f| match f.role {
            Role::Pad { cell } => cell,
            _ => None,
        });
        let descending = self.fingers.iter().any(|f| matches!(f.role, Role::Descend));
        let forward = under.is_some_and(Cell::forward);
        for cell in Cell::ALL {
            if cell.diagonal() && !forward {
                continue;
            }
            let pressed = under == Some(cell) || (cell == Cell::Middle && (sneaking || descending));
            let (icon, turn) = match cell {
                Cell::Forward => (ARROW, Turn::Up),
                Cell::Back => (ARROW, Turn::Down),
                Cell::Left => (ARROW, Turn::Left),
                Cell::Right => (ARROW, Turn::Right),
                Cell::ForwardLeft => (DIAGONAL, Turn::Up),
                Cell::ForwardRight => (DIAGONAL, Turn::Mirror),
                Cell::Middle if self.context.flying => (JUMP, Turn::Down),
                Cell::Middle => (SNEAK, Turn::Up),
            };
            pen.button(pen.layout.cell(cell), pressed, icon, turn);
        }
    }

    fn draw_joystick(&self, pen: &mut Pen<'_>) {
        let layout = pen.layout;
        let origin = self.fingers.iter().find_map(|f| match f.role {
            Role::Stick { origin } => Some(origin),
            _ => None,
        });
        let centre = origin.unwrap_or_else(|| layout.stick_home());
        let radius = layout.reach() / layout.unit;
        pen.disc(centre, radius, OUTLINE);
        pen.disc(centre, radius - 1.0, PLATE);
        let knob = [
            centre[0] + self.stick[0] * layout.reach(),
            centre[1] - self.stick[1] * layout.reach(),
        ];
        pen.disc(knob, KNOB / 2.0, OUTLINE);
        let lit = if self.sprinting {
            0xffff_ffff
        } else {
            pressed_plate()
        };
        pen.disc(knob, KNOB / 2.0 - 1.0, lit);
    }

    /// The menu fingers' marks: the ring filling under a finger whose hold will do something,
    /// and the drop hint above a finger that would throw a stack.
    fn draw_menu(&self, pen: &mut Pen<'_>, now: Instant) {
        for finger in &self.fingers {
            match finger.role {
                Role::Menu(Gesture::Pending {
                    landed,
                    from,
                    here,
                    held: false,
                }) if hold(&here) != Hold::Nothing => {
                    pen.ring(from, now.duration_since(landed), MENU_HOLD);
                }
                Role::Menu(Gesture::Pressing {
                    outside: Some(at), ..
                }) => {
                    let hint = pen.layout.above([at[0] as f32, at[1] as f32]);
                    pen.button(hint, false, DROP, Turn::Up);
                }
                _ => {}
            }
        }
    }

    /// A ring of dots filling around each finger held still on the view, until it mines or uses.
    fn draw_holds(&self, pen: &mut Pen<'_>, now: Instant) {
        for finger in &self.fingers {
            if let Role::Look {
                landed,
                last,
                still: true,
                holding: None,
                ..
            } = finger.role
            {
                pen.ring(last, now.duration_since(landed), HOLD);
            }
        }
    }

    fn on_control(&self, control: &Control) -> bool {
        self.fingers
            .iter()
            .any(|f| matches!(f.role, Role::Function(c) if c.place == control.place))
    }

    /// Play's key caps, with their keys on them.
    fn draw_caps(&self, pen: &mut Pen<'_>) {
        let g = pen.layout.gui;
        for control in self.live_controls() {
            let r = pen.layout.control(control.place);
            pen.plate(r, self.on_control(control));
            let top = r[1] + (r[3] - CAPITALS * g) / 2.0;
            pen.text(control.keys, r, top, u32::MAX);
        }
    }

    /// The Fn panel over play, which it shades: Back, and a tile for each key with what it does.
    fn draw_panel(&self, pen: &mut Pen<'_>) {
        let layout = pen.layout;
        let g = layout.gui;
        pen.fill(layout.screen(), SHADE);
        pen.button(layout.button(Button::Back), false, ARROW, Turn::Left);
        pen.plate(layout.panel(), false);
        for control in self.live_controls() {
            let r = layout.control(control.place);
            pen.plate(r, self.on_control(control));
            let top = r[1] + (r[3] - (LINE + CAPITALS) * g) / 2.0;
            pen.text(control.keys, r, top, rgba(255, 255, 85, 255));
            pen.text(control.does, r, top + LINE * g, u32::MAX);
        }
    }
}
