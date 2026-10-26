//! Where the touch controls are, in physical pixels: Pocket Edition's layout, sized in points so
//! a button is as big under the thumb on an iPhone as on an iPad.

use super::functions::Place;
use crate::hud::HudBatch;

/// x, y, width, height in physical pixels.
pub(super) type Rect = [f32; 4];

/// Physical pixels per art pixel for each point: buttons are drawn in pixel art, and an 18-pixel
/// button is then about 50 points, a comfortable thumb.
const UNIT_PER_POINT: f64 = 2.75;
/// Sizes in art pixels.
pub(super) const CELL: f32 = 18.0;
const PITCH: f32 = 20.0;
const JUMP: f32 = 22.0;
const SMALL: f32 = 14.0;
const MARGIN: f32 = 8.0;
/// A fingertip's reach above the point it touches.
const FINGER: f32 = 12.0;
/// The joystick's reach, and its knob's size.
pub(super) const REACH: f32 = 22.0;
pub(super) const KNOB: f32 = 14.0;
/// The joystick's share of the screen, from the left edge.
const STICK_SHARE: f32 = 0.4;
/// A key cap's least side in GUI pixels, which its keys are written in: two of the game font's
/// characters and a margin.
const CAP_TEXT: f32 = 16.0;
/// The Fn panel's tiles, in GUI pixels: two lines of the game font, as wide as its longest label.
const TILE: [f32; 2] = [100.0, 26.0];
const TILE_GAP: f32 = 4.0;
const PANEL_PADDING: f32 = 6.0;
const PANEL_COLUMNS: u8 = 2;
const PANEL_ROWS: u8 = 3;

/// A square of the D-pad: its arrows, the forward diagonals that show while forward is held,
/// and the middle (sneak, or down while flying).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cell {
    Forward,
    Back,
    Left,
    Right,
    ForwardLeft,
    ForwardRight,
    Middle,
}

impl Cell {
    pub(super) const ALL: [Cell; 7] = [
        Cell::Forward,
        Cell::Back,
        Cell::Left,
        Cell::Right,
        Cell::ForwardLeft,
        Cell::ForwardRight,
        Cell::Middle,
    ];
    fn at(col: i32, row: i32) -> Option<Cell> {
        Some(match (col, row) {
            (0, 0) => Cell::ForwardLeft,
            (1, 0) => Cell::Forward,
            (2, 0) => Cell::ForwardRight,
            (0, 1) => Cell::Left,
            (1, 1) => Cell::Middle,
            (2, 1) => Cell::Right,
            (1, 2) => Cell::Back,
            _ => return None,
        })
    }
    fn grid(self) -> (f32, f32) {
        match self {
            Cell::ForwardLeft => (0.0, 0.0),
            Cell::Forward => (1.0, 0.0),
            Cell::ForwardRight => (2.0, 0.0),
            Cell::Left => (0.0, 1.0),
            Cell::Middle => (1.0, 1.0),
            Cell::Right => (2.0, 1.0),
            Cell::Back => (1.0, 2.0),
        }
    }
    /// The move vector: x right, y forward.
    pub(super) fn vector(self) -> [f32; 2] {
        const D: f32 = std::f32::consts::FRAC_1_SQRT_2;
        match self {
            Cell::Forward => [0.0, 1.0],
            Cell::Back => [0.0, -1.0],
            Cell::Left => [-1.0, 0.0],
            Cell::Right => [1.0, 0.0],
            Cell::ForwardLeft => [-D, D],
            Cell::ForwardRight => [D, D],
            Cell::Middle => [0.0, 0.0],
        }
    }
    pub(super) fn forward(self) -> bool {
        matches!(self, Cell::Forward | Cell::ForwardLeft | Cell::ForwardRight)
    }
    pub(super) fn diagonal(self) -> bool {
        matches!(self, Cell::ForwardLeft | Cell::ForwardRight)
    }
}

/// The buttons outside the D-pad.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Button {
    Jump,
    Inventory,
    Pause,
    /// Escape, on a world's menus.
    Back,
    /// The keyboard's Shift, in inventories: bottom left, under the left thumb.
    Shift,
    /// Opens chat, in play: left of Pause.
    Chat,
}

#[derive(Clone, Copy, Debug)]
pub struct Layout {
    size: [f32; 2],
    /// Physical pixels per GUI pixel (the HUD's) and per art pixel.
    pub(super) gui: f32,
    pub(super) unit: f32,
}

impl Layout {
    /// A screen of `width` x `height` physical pixels, `scale_factor` of them to a point.
    pub fn new(width: u32, height: u32, scale_factor: f64) -> Self {
        Self {
            size: [width as f32, height as f32],
            gui: HudBatch::new([width, height]).scale,
            unit: (scale_factor * UNIT_PER_POINT).round().max(2.0) as f32,
        }
    }
    fn u(&self, art: f32) -> f32 {
        art * self.unit
    }
    /// The hotbar (`Hud.extractItemHotbar`).
    pub(super) fn hotbar(&self) -> Rect {
        let w = (self.size[0] / self.gui).ceil() as i32;
        let h = (self.size[1] / self.gui).ceil() as i32;
        [
            (w / 2 - 91) as f32 * self.gui,
            (h - 22) as f32 * self.gui,
            182.0 * self.gui,
            22.0 * self.gui,
        ]
    }
    pub(super) fn hotbar_slot(&self, at: [f32; 2]) -> Option<u8> {
        let [x, y, w, _] = self.hotbar();
        ((x..x + w).contains(&at[0]) && at[1] >= y).then(|| {
            ((at[0] - x) / self.gui - 1.0)
                .div_euclid(20.0)
                .clamp(0.0, 8.0) as u8
        })
    }
    /// The D-pad's 3 x 3 squares in the bottom-left corner, raised above the hotbar where a
    /// narrow screen brings them together.
    pub(super) fn dpad(&self) -> Rect {
        let side = self.u(2.0 * PITCH + CELL);
        let x = self.u(MARGIN);
        let mut y = self.size[1] - self.u(MARGIN) - side;
        let [hx, hy, ..] = self.hotbar();
        if x + side > hx {
            y = y.min(hy - self.u(MARGIN) - side);
        }
        [x, y, side, side]
    }
    pub(super) fn cell(&self, cell: Cell) -> Rect {
        let [x, y, ..] = self.dpad();
        let (col, row) = cell.grid();
        let pitch = self.u(PITCH);
        [x + col * pitch, y + row * pitch, self.u(CELL), self.u(CELL)]
    }
    /// The D-pad square under `at`, if `at` is on the D-pad.
    pub(super) fn cell_at(&self, at: [f32; 2]) -> Option<Cell> {
        let [x, y, side, _] = self.dpad();
        if !(x..x + side).contains(&at[0]) || !(y..y + side).contains(&at[1]) {
            return None;
        }
        let pitch = self.u(PITCH);
        let col = (((at[0] - x) / pitch) as i32).min(2);
        let row = (((at[1] - y) / pitch) as i32).min(2);
        Cell::at(col, row)
    }
    pub(super) fn button(&self, button: Button) -> Rect {
        let [w, h] = self.size;
        match button {
            Button::Jump => {
                let side = self.u(JUMP);
                [
                    w - self.u(MARGIN) - side,
                    h - self.u(MARGIN) - side,
                    side,
                    side,
                ]
            }
            Button::Inventory => {
                let [x, y, width, height] = self.hotbar();
                [x + width + self.u(2.0), y, height, height]
            }
            Button::Pause => {
                let side = self.u(SMALL);
                [w - self.u(MARGIN) - side, self.u(MARGIN), side, side]
            }
            Button::Back => {
                let side = self.u(SMALL);
                [self.u(MARGIN), self.u(MARGIN), side, side]
            }
            Button::Shift => {
                let side = self.u(CELL);
                [self.u(MARGIN), h - self.u(MARGIN) - side, side, side]
            }
            Button::Chat => {
                let side = self.u(SMALL);
                let pause = self.button(Button::Pause);
                [pause[0] - self.u(2.0) - side, self.u(MARGIN), side, side]
            }
        }
    }
    pub(super) fn on(&self, button: Button, at: [f32; 2]) -> bool {
        contains(self.button(button), at)
    }
    /// A key cap in a row along the top-left corner, or a tile of the panel.
    pub(super) fn control(&self, place: Place) -> Rect {
        match place {
            Place::Cap(i) => {
                let side = self.u(CELL).max(CAP_TEXT * self.gui);
                let x = self.u(MARGIN) + f32::from(i) * (side + self.u(2.0));
                [x, self.u(MARGIN), side, side]
            }
            Place::Tile(i) => {
                let [x, y, ..] = self.panel();
                let (col, row) = (f32::from(i % PANEL_COLUMNS), f32::from(i / PANEL_COLUMNS));
                let [w, h] = TILE;
                [
                    x + (PANEL_PADDING + col * (w + TILE_GAP)) * self.gui,
                    y + (PANEL_PADDING + row * (h + TILE_GAP)) * self.gui,
                    w * self.gui,
                    h * self.gui,
                ]
            }
        }
    }
    /// The Fn panel, in the middle of the screen on whole GUI pixels.
    pub(super) fn panel(&self) -> Rect {
        let span = |n: u8, tile: f32| {
            let n = f32::from(n);
            n * tile + (n - 1.0) * TILE_GAP + 2.0 * PANEL_PADDING
        };
        let (w, h) = (span(PANEL_COLUMNS, TILE[0]), span(PANEL_ROWS, TILE[1]));
        let x = ((self.size[0] / self.gui - w) / 2.0).floor();
        let y = ((self.size[1] / self.gui - h) / 2.0).floor();
        [x * self.gui, y * self.gui, w * self.gui, h * self.gui]
    }
    /// The whole screen.
    pub(super) fn screen(&self) -> Rect {
        [0.0, 0.0, self.size[0], self.size[1]]
    }
    /// The joystick's side of the screen, and where it rests: the D-pad's middle.
    pub(super) fn stick_zone(&self, at: [f32; 2]) -> bool {
        at[0] < self.size[0] * STICK_SHARE
    }
    pub(super) fn stick_home(&self) -> [f32; 2] {
        let [x, y, side, _] = self.dpad();
        [x + side / 2.0, y + side / 2.0]
    }
    pub(super) fn reach(&self) -> f32 {
        self.u(REACH)
    }
    /// How far a finger may drift and still tap or hold.
    pub(super) fn slop(&self) -> f32 {
        self.u(4.0)
    }
    /// How far above a finger things stay in sight: a fingertip, about 33 points.
    pub(super) fn finger_clearance(&self) -> f32 {
        self.u(FINGER)
    }
    /// A small button's square over a finger at `at`, clear of the fingertip, kept on screen.
    pub(super) fn above(&self, at: [f32; 2]) -> Rect {
        let side = self.u(SMALL);
        let x = (at[0] - side / 2.0).clamp(0.0, self.size[0] - side);
        let y = (at[1] - self.finger_clearance() - side).max(0.0);
        [x, y, side, side]
    }
}

pub(super) fn contains(r: Rect, at: [f32; 2]) -> bool {
    (r[0]..r[0] + r[2]).contains(&at[0]) && (r[1]..r[1] + r[3]).contains(&at[1])
}

#[cfg(test)]
mod tests {
    use super::super::functions::CONTROLS;
    use super::*;

    /// Phones and tablets in landscape, with their pixels per point: an iPhone SE, an iPhone,
    /// an iPhone Pro Max, the iPhone Duo unfolded, an iPad mini and a 13-inch iPad Pro.
    const SCREENS: [(u32, u32, f64); 6] = [
        (1334, 750, 2.0),
        (2622, 1206, 3.0),
        (2868, 1320, 3.0),
        (2853, 2007, 3.0),
        (2266, 1488, 2.0),
        (2752, 2064, 2.0),
    ];

    fn overlap(a: Rect, b: Rect) -> bool {
        a[0] < b[0] + b[2] && b[0] < a[0] + a[2] && a[1] < b[1] + b[3] && b[1] < a[1] + a[3]
    }

    #[test]
    fn the_controls_fit_every_screen_without_overlapping() {
        for (w, h, scale) in SCREENS {
            let layout = Layout::new(w, h, scale);
            let mut rects = vec![
                ("D-pad", layout.dpad()),
                ("hotbar", layout.hotbar()),
                ("jump", layout.button(Button::Jump)),
                ("inventory", layout.button(Button::Inventory)),
                ("pause", layout.button(Button::Pause)),
                ("chat", layout.button(Button::Chat)),
            ];
            rects.extend(
                CONTROLS
                    .iter()
                    .filter(|c| matches!(c.place, Place::Cap(_)))
                    .map(|c| (c.keys, layout.control(c.place))),
            );
            let slack = layout.gui;
            for (name, r) in &rects {
                assert!(
                    r[0] >= 0.0
                        && r[1] >= 0.0
                        && r[0] + r[2] <= w as f32
                        && r[1] + r[3] <= h as f32 + slack,
                    "{w}x{h}: the {name} leaves the screen"
                );
            }
            while let Some((a, ra)) = rects.pop() {
                for (b, rb) in &rects {
                    assert!(!overlap(ra, *rb), "{w}x{h}: the {a} and the {b} overlap");
                }
            }
        }
    }

    #[test]
    fn the_fn_panel_fits_every_screen_clear_of_back_with_its_tiles_apart() {
        for (w, h, scale) in SCREENS {
            let layout = Layout::new(w, h, scale);
            let panel = layout.panel();
            let inside = |r: Rect, of: Rect| {
                r[0] >= of[0]
                    && r[1] >= of[1]
                    && r[0] + r[2] <= of[0] + of[2]
                    && r[1] + r[3] <= of[1] + of[3]
            };
            assert!(
                inside(panel, layout.screen()),
                "{w}x{h}: the panel leaves the screen"
            );
            assert!(
                !overlap(panel, layout.button(Button::Back)),
                "{w}x{h}: the panel on Back"
            );
            let mut tiles: Vec<_> = CONTROLS
                .iter()
                .filter(|c| matches!(c.place, Place::Tile(_)))
                .map(|c| (c.keys, layout.control(c.place)))
                .collect();
            assert_eq!(tiles.len(), 6);
            while let Some((a, ra)) = tiles.pop() {
                assert!(inside(ra, panel), "{w}x{h}: {a} leaves the panel");
                for (b, rb) in &tiles {
                    assert!(!overlap(ra, *rb), "{w}x{h}: {a} and {b} overlap");
                }
            }
        }
    }

    #[test]
    fn shift_stays_clear_of_back_and_of_an_inventory_with_its_recipe_book() {
        for (w, h, scale) in SCREENS {
            let layout = Layout::new(w, h, scale);
            let shift = layout.button(Button::Shift);
            let g = layout.gui;
            // The crafting book (147 GUI pixels) beside the inventory (176), which it pushes right.
            let (gw, gh) = (w as f32 / g, h as f32 / g);
            let panel = [
                (gw / 2.0 - 88.0 - 77.0) * g,
                (gh / 2.0 - 83.0) * g,
                323.0 * g,
                166.0 * g,
            ];
            assert!(
                shift[0] >= 0.0 && shift[1] + shift[3] <= h as f32,
                "{w}x{h}: Shift leaves the screen"
            );
            assert!(
                !overlap(shift, layout.button(Button::Back)),
                "{w}x{h}: Shift on Back"
            );
            assert!(!overlap(shift, panel), "{w}x{h}: Shift on the inventory");
        }
    }

    #[test]
    fn a_finger_on_the_dpad_finds_its_square() {
        let layout = Layout::new(2622, 1206, 3.0);
        for cell in Cell::ALL {
            let [x, y, s, _] = layout.cell(cell);
            assert_eq!(layout.cell_at([x + s / 2.0, y + s / 2.0]), Some(cell));
        }
        let [x, y, side, _] = layout.dpad();
        assert_eq!(layout.cell_at([x + 1.0, y + side - 1.0]), None);
        assert_eq!(layout.cell_at([x + side + 1.0, y]), None);
    }

    #[test]
    fn buttons_are_a_thumb_wide_on_phones_and_tablets() {
        for (w, h, scale) in SCREENS {
            let layout = Layout::new(w, h, scale);
            let points = layout.cell(Cell::Forward)[2] / scale as f32;
            assert!((44.0..=60.0).contains(&points), "{w}x{h}: {points} pt");
        }
    }
}
