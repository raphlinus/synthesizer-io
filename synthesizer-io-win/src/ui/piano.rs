// Copyright 2018 The Synthesizer IO Authors.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Piano keyboard widget.

use direct2d::brush::SolidColorBrush;
use direct2d::RenderTarget;

use druid::widget::Widget;
use druid::MouseEvent;
use druid::{BoxConstraints, Geometry, LayoutResult, Ui};
use druid::{HandlerCtx, Id, LayoutCtx, PaintCtx};

use synthesizer_io_core::engine::NoteEvent;

pub struct Piano {
    start_note: u8,
    end_note: u8,

    pressed: [bool; 128],
    // Note corresponding to mouse press.
    cur_note: Option<u8>,

    // Note: we could probably eliminate this if we had access to size
    // in HandlerCtx. Alternatively, we could precompute width_scale.
    size: (f32, f32),
}

const OCTAVE_WIDTH: i32 = 14;

const NOTE_POS: &[(u8, u8)] = &[
    (0, 0),
    (1, 1),
    (2, 0),
    (3, 1),
    (4, 0),
    (6, 0),
    (7, 1),
    (8, 0),
    (9, 1),
    (10, 0),
    (11, 1),
    (12, 0),
];

const INSET: f32 = 2.0;

impl Widget for Piano {
    fn paint(&mut self, paint_ctx: &mut PaintCtx, geom: &Geometry) {
        let rt = paint_ctx.render_target();
        let black = SolidColorBrush::create(rt)
            .with_color(0x080800)
            .build()
            .unwrap();
        let white = SolidColorBrush::create(rt)
            .with_color(0xf0f0ea)
            .build()
            .unwrap();
        let active = SolidColorBrush::create(rt)
            .with_color(0x107010)
            .build()
            .unwrap();
        let (x, y) = geom.pos;

        for note in self.start_note..self.end_note {
            let (u0, v0, u1, v1) = self.note_geom(note);
            let color = if self.pressed[note as usize] {
                &active
            } else {
                if v0 == 0.0 {
                    &black
                } else {
                    &white
                }
            };
            let x0 = x + u0 * geom.size.0 + INSET;
            let y0 = y + v0 * geom.size.1 + INSET;
            let x1 = x + u1 * geom.size.0 - INSET;
            let y1 = y + v1 * geom.size.1 - INSET;

            rt.fill_rectangle((x0, y0, x1, y1), color);
        }
    }

    fn layout(
        &mut self,
        bc: &BoxConstraints,
        _children: &[Id],
        _size: Option<(f32, f32)>,
        _ctx: &mut LayoutCtx,
    ) -> LayoutResult {
        let size = bc.constrain((100.0, 100.0));
        self.size = size;
        LayoutResult::Size(size)
    }

    fn mouse(&mut self, event: &MouseEvent, ctx: &mut HandlerCtx) -> bool {
        if event.count > 0 {
            ctx.set_active(true);
            let u = event.x / self.size.0;
            let v = event.y / self.size.1;
            for note in self.start_note..self.end_note {
                let (u0, v0, u1, v1) = self.note_geom(note);
                if u >= u0 && u < u1 && v >= v0 && v < v1 {
                    self.cur_note = Some(note);
                    break;
                }
            }
            if let Some(note) = self.cur_note {
                self.pressed[note as usize] = true;
                ctx.send_event(NoteEvent {
                    down: true,
                    note: note,
                    velocity: 100,
                });
                ctx.invalidate();
            }
        } else {
            ctx.set_active(false);
            if let Some(note) = self.cur_note {
                self.pressed[note as usize] = false;
                ctx.send_event(NoteEvent {
                    down: false,
                    note: note,
                    velocity: 0,
                });
                ctx.invalidate();
            }
            self.cur_note = None;
        }
        true
    }
}

impl Piano {
    pub fn new() -> Piano {
        Piano {
            start_note: 48,
            end_note: 72,
            pressed: [false; 128],
            cur_note: None,
            size: (0.0, 0.0),
        }
    }

    pub fn ui(self, ctx: &mut Ui) -> Id {
        ctx.add(self, &[])
    }

    fn note_pos(&self, note: u8) -> (i32, i32) {
        let octave = note / 12;
        let (x, y) = NOTE_POS[(note % 12) as usize];
        (OCTAVE_WIDTH * (octave as i32) + (x as i32), y as i32)
    }

    // Geometry is in unit square
    fn note_geom(&self, note: u8) -> (f32, f32, f32, f32) {
        let start_x = self.note_pos(self.start_note).0;
        let width = self.note_pos(self.end_note - 1).0 - start_x + 2;
        let width_scale = 1.0 / (width as f32);
        let (x, y) = self.note_pos(note);
        let u = (x - start_x) as f32 * width_scale;
        let v = y as f32 * 0.5;
        (u, 0.5 - v, 2.0 * width_scale + u, 1.0 - v)
    }
}
