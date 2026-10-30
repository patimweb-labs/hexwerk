//! Nyan-Cat als Ladeanzeige im Fuss.
//!
//! Quelle ist der CodePen "CSS Nyan Cat" von eusonic
//! (<https://codepen.io/eusonic/pen/nrjqKn>), Skalierung 1 Bildpunkt = 1 px.
//! Die Raster unten sind aus dessen CSS-Ebenen erzeugt, nicht abgezeichnet:
//! jede Ebene des Pen (Pop-Tart, Kopf, Beine, Schwanz, Regenbogen) ist ein
//! Farbband an fester Stelle mit fester Groesse. Ein Auswerter hat diese Ebenen
//! so gestapelt wie der Browser und das Ergebnis als Zeichenraster geschrieben.
//!
//! Bewegung ebenfalls aus dem Pen: fuenf Zyklen mit je sechs Bildern
//! (catCycle, headCycle, feetCycle, feetSpriteCycle, tailCycle) und der
//! Regenbogen mit zwoelf Bildern, Schritt 83 ms wie dort bei
//! `animation-timing-function: step-end`.

use gtk::prelude::*;
use std::cell::Cell;
use std::rc::Rc;

/// Rahmen der Anzeige: Regenbogen plus Katze (26 x 20 wie im Pen).
const W: i32 = 96;
const H: i32 = 26;
/// Ein Bild des Zyklus. Der Pen laeuft 0,5 s je Zyklus bei sechs Bildern.
const STEP_MS: u64 = 83;

#[derive(Clone)]
pub struct NyanCat {
    area: gtk::DrawingArea,
    tick: Rc<Cell<u64>>,
    running: Rc<Cell<bool>>,
}

impl NyanCat {
    pub fn new() -> Self {
        let area = gtk::DrawingArea::new();
        area.set_size_request(W, H);
        area.set_valign(gtk::Align::Center);
        // Luft nach links und rechts: sonst klebt der Regenbogen am Rand der
        // Wortmarke und die Katze am Zustand.
        area.set_margin_start(5);
        area.set_margin_end(5);
        area.set_tooltip_text(Some("Katalog wird geladen"));

        let tick = Rc::new(Cell::new(0u64));
        let tick_draw = Rc::clone(&tick);
        area.set_draw_func(move |_, ctx, width, height| {
            draw(ctx, width as f64, height as f64, tick_draw.get());
        });

        Self {
            area,
            tick,
            running: Rc::new(Cell::new(false)),
        }
    }

    pub fn widget(&self) -> &gtk::DrawingArea {
        &self.area
    }

    pub fn set_running(&self, running: bool) {
        if !running {
            self.running.set(false);
            self.tick.set(0);
            self.area.queue_draw();
            return;
        }
        if self.running.get() {
            return;
        }
        self.running.set(true);
        let tick = Rc::clone(&self.tick);
        let area = self.area.clone();
        let laeuft = Rc::clone(&self.running);
        glib::timeout_add_local(std::time::Duration::from_millis(STEP_MS), move || {
            if !laeuft.get() {
                return glib::ControlFlow::Break;
            }
            tick.set(tick.get() + 1);
            area.queue_draw();
            glib::ControlFlow::Continue
        });
    }
}

/// Farbzeichen des Pen.
fn color(cell: char) -> Option<&'static str> {
    match cell {
        '#' => Some("#000000"),
        't' => Some("#ffcc99"),
        'P' => Some("#ff99ff"),
        'm' => Some("#ff3399"),
        'g' => Some("#999999"),
        'p' => Some("#ff9999"),
        'w' => Some("#ffffff"),
        'r' => Some("#ff0000"),
        'o' => Some("#ff9900"),
        'y' => Some("#ffff00"),
        'G' => Some("#33ff00"),
        'l' => Some("#0099ff"),
        'b' => Some("#6633ff"),
        _ => None,
    }
}

fn rgb(hex: &str) -> (f64, f64, f64) {
    let raw = hex.trim_start_matches('#');
    let part = |slice: &str| u8::from_str_radix(slice, 16).unwrap_or(0) as f64 / 255.0;
    (part(&raw[0..2]), part(&raw[2..4]), part(&raw[4..6]))
}

fn pixel(ctx: &gtk::cairo::Context, hex: &str, x: f64, y: f64) {
    let (r, g, b) = rgb(hex);
    ctx.set_source_rgb(r, g, b);
    ctx.rectangle(x.round(), y.round(), 1.0, 1.0);
    let _ = ctx.fill();
}

fn blit(ctx: &gtk::cairo::Context, art: &[&str], x: f64, y: f64) {
    for (row, line) in art.iter().enumerate() {
        for (col, cell) in line.chars().enumerate() {
            if let Some(hex) = color(cell) {
                pixel(ctx, hex, x + col as f64, y + row as f64);
            }
        }
    }
}

/// Fenster auf ein laengeres Sprite (Beine, Schwanz).
fn blit_window(ctx: &gtk::cairo::Context, art: &[&str], x: f64, y: f64, first: usize, rows: usize) {
    for offset in 0..rows {
        let Some(line) = art.get(first + offset) else {
            break;
        };
        for (col, cell) in line.chars().enumerate() {
            if let Some(hex) = color(cell) {
                pixel(ctx, hex, x + col as f64, y + offset as f64);
            }
        }
    }
}

fn draw(ctx: &gtk::cairo::Context, w: f64, h: f64, tick: u64) {
    ctx.set_antialias(gtk::cairo::Antialias::None);

    let frame = (tick % 6) as usize;
    let rainbow_frame = (tick % 12) as usize;

    // 28 statt 26: der Kopf zittert in manchen Bildern einen Punkt nach rechts
    // und wurde sonst am Rand des Zeichenbereichs abgeschnitten.
    let cat_w = 28.0;
    let cat_h = 20.0;
    let cat_x = (w - cat_w).round();
    let cat_y = ((h - cat_h) / 2.0).round();

    // catCycle: zwei Bilder oben, vier einen Punkt tiefer.
    let cat_dy = if frame < 2 { 0.0 } else { 1.0 };

    // ---- Regenbogen: Kachel 16 x 19, alle zwei Bilder um 9 Punkte versetzt ----
    let shift = if (rainbow_frame / 2) % 2 == 0 {
        0.0
    } else {
        -9.0
    };
    let rainbow_top = cat_y + 0.5 + cat_dy;
    let mut x = shift;
    while x < cat_x + 8.0 {
        for (row, line) in RAINBOW.iter().enumerate() {
            for (col, cell) in line.chars().enumerate() {
                let px = x + col as f64;
                if px < 0.0 || px > cat_x + 8.0 {
                    continue;
                }
                if let Some(hex) = color(cell) {
                    pixel(ctx, hex, px, rainbow_top + row as f64);
                }
            }
        }
        x += 16.0;
    }

    blit(ctx, &POPTART, cat_x, cat_y + cat_dy);

    // headCycle: Bilder 2 bis 4 einen Punkt nach rechts, Bild 6 einen nach oben.
    let (head_dx, head_dy) = match frame {
        1 | 2 | 3 => (1.0, 0.0),
        5 => (0.0, -1.0),
        _ => (0.0, 0.0),
    };
    blit(
        ctx,
        &HEAD,
        cat_x + 10.0 + head_dx,
        cat_y + 5.0 + cat_dy + head_dy,
    );

    // feetSpriteCycle und feetCycle.
    let feet_row = match frame {
        0 => 0,
        1 | 2 | 3 | 4 => 5,
        _ => 10,
    };
    let feet_dx = match frame {
        1 => 1.0,
        2 => 2.0,
        3 => 1.0,
        4 | 5 => -1.0,
        _ => 0.0,
    };
    blit_window(
        ctx,
        &FEET,
        cat_x - 2.0 + feet_dx,
        cat_y + 15.0 + cat_dy,
        feet_row,
        5,
    );

    // tailSpriteCycle und tailCycle.
    let tail_row = match frame {
        1 => 7,
        2 => 14,
        3 => 21,
        4 => 28,
        _ => 0,
    };
    let tail_dy = match frame {
        2 => 1.0,
        3 => 2.0,
        4 | 5 => -1.0,
        _ => 0.0,
    };
    blit_window(
        ctx,
        &TAIL,
        cat_x - 7.0,
        cat_y + 7.0 + cat_dy + tail_dy,
        tail_row,
        7,
    );
}

/// Regenbogen-Kachel 16 x 19, sechs Farben zu drei Zeilen.
const RAINBOW: [&str; 19] = [
    "rrrrrrrr........",
    "rrrrrrrrrrrrrrrr",
    "rrrrrrrrrrrrrrrr",
    "oooooooorrrrrrrr",
    "oooooooooooooooo",
    "oooooooooooooooo",
    "yyyyyyyyoooooooo",
    "yyyyyyyyyyyyyyyy",
    "yyyyyyyyyyyyyyyy",
    "GGGGGGGGyyyyyyyy",
    "GGGGGGGGGGGGGGGG",
    "GGGGGGGGGGGGGGGG",
    "llllllllGGGGGGGG",
    "llllllllllllllll",
    "llllllllllllllll",
    "bbbbbbbbllllllll",
    "bbbbbbbbbbbbbbbb",
    "bbbbbbbbbbbbbbbb",
    "........bbbbbbbb",
];

/// Pop-Tart 21 x 18.
const POPTART: [&str; 18] = [
    "..#################..",
    "#ttttttttttttttttttt#",
    "#tttPPPPPPPPPPPPPttt#",
    "#ttPPPPPPmPPmPPPPPtt#",
    "#tPPmPPPPPPPPPPPPPPt#",
    "#tPPPPPPPPPPPPPPmPPt#",
    "#tPPPPPPPPPPPPPPPPPt#",
    "#tPPPPPPmPPPPPPPPPPt#",
    "#tPPPPPPPPPPPPPPPPPt#",
    "#tPPPmPPPPPPPPPPPPPt#",
    "#tPPPPPPPmPPPPPPPPPt#",
    "#tPmPPPPPPPPPPPPPPPt#",
    "#tPPPPPPPPPPPPPPPPPt#",
    "#tPPPPPmPPPPPPPPPPPt#",
    "#ttPmPPPPPPmPPPPPPtt#",
    "#tttPPPPPPPPPPPPPttt#",
    "#ttttttttttttttttttt#",
    "..#################..",
];

/// Kopf 16 x 13.
const HEAD: [&str; 13] = [
    "..##........##..",
    ".#gg#......#gg#.",
    ".#ggg#....#ggg#.",
    ".#gggg####gggg#.",
    ".#gggggggggggg#.",
    "#gggggggggggggg#",
    "#gggw#gggggw#gg#",
    "#ggg##ggg#g##gg#",
    "#gppgggggggggpp#",
    "#gppg#gg#gg#gpp#",
    ".#ggg#######gg#.",
    "..#gggggggggg#..",
    "...##########...",
];

/// Beine 24 x 15: drei Streifen zu je fuenf Zeilen.
const FEET: [&str; 15] = [
    "..##g...................",
    "###ggg.............ggg#.",
    "gggg##gggg#...#ggg##ggg#",
    "ggg##.#gg#.....#gg#.#gg#",
    "####..##........###..##.",
    "..##g...................",
    "##gggg.............ggg#.",
    "gggg##ggg#....#ggg##ggg#",
    "ggg#.#gg#......#gg#.#gg#",
    "###...###.......###..###",
    "..##g...................",
    "##gggg.............ggg#.",
    "gggg##ggg#....#ggg##ggg#",
    "ggg#.#gg#......#gg#.#gg#",
    "###..###.......###...###",
];

/// Schwanz 7 x 35: fuenf Bilder zu je sieben Zeilen.
const TAIL: [&str; 35] = [
    "#####..", "##gg##.", "###gg##", "..##gg#", "...##gg", "....###", "......#", ".......",
    "..##...", "##gg#..", "##gg###", "..#gggg", "...##gg", ".....##", ".......", ".......",
    "......#", "...####", "###gggg", "##ggg##", "..####.", ".......", ".....##", "...##gg",
    "..#gggg", "##gg###", "##gg#..", "..##...", ".......", ".......", "#####..", "gggg###",
    "##gggg#", "..####g", ".....##",
];
