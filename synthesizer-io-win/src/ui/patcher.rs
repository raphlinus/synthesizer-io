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

//! Widget representing patcher surface.

use std::collections::HashSet;

use itertools::Itertools;

use direct2d::brush::SolidColorBrush;
use direct2d::RenderTarget;

use xi_win_ui::{BoxConstraints, Geometry, LayoutResult, UiInner};
use xi_win_ui::{Id, HandlerCtx, LayoutCtx, PaintCtx};
use xi_win_ui::MouseEvent;
use xi_win_ui::widget::Widget;

pub struct Patcher {
    size: (f32, f32),
    grid_size: (usize, usize),
    offset: (f32, f32),
    scale: f32,

    grid: WireGrid,
    last_xy: Option<(f32, f32)>,
    draw_mode: Option<bool>,
}

#[derive(Default)]
pub struct WireGrid {
    grid: HashSet<(u16, u16, bool)>,
}

impl Widget for Patcher {
    fn paint(&mut self, paint_ctx: &mut PaintCtx, geom: &Geometry) {
        let rt = paint_ctx.render_target();
        // TODO: clip to geom
        let grid_color = SolidColorBrush::create(rt).with_color(0x405070).build().unwrap();
        let wire_color = SolidColorBrush::create(rt).with_color(0x808080).build().unwrap();
        let x0 = geom.pos.0 + self.offset.0;
        let y0 = geom.pos.1 + self.offset.1;
        for i in 0..(self.grid_size.0 + 1) {
            rt.draw_line((x0 + self.scale * (i as f32), y0),
                (x0 + self.scale * (i as f32), y0 + self.scale * (self.grid_size.1 as f32)),
                &grid_color, 1.0, None);
        }
        for i in 0..(self.grid_size.1 + 1) {
            rt.draw_line((x0, y0 + self.scale * (i as f32)),
                (x0 + self.scale * (self.grid_size.0 as f32), y0 + self.scale * (i as f32)),
                &grid_color, 1.0, None);
        }
        for (i, j, vert) in &self.grid.grid {
            let x = x0 + (*i as f32 + 0.5) * self.scale;
            let y = y0 + (*j as f32 + 0.5) * self.scale;
            let (x1, y1) = if *vert { (x, y + self.scale) } else { (x + self.scale, y) };
            rt.draw_line((x, y), (x1, y1), &wire_color, 3.0, None);
        }
    }

    fn layout(&mut self, bc: &BoxConstraints, _children: &[Id], _size: Option<(f32, f32)>,
        _ctx: &mut LayoutCtx) -> LayoutResult
    {
        let size = bc.constrain((100.0, 100.0));
        self.size = size;
        LayoutResult::Size(size)
    }

    fn mouse(&mut self, event: &MouseEvent, ctx: &mut HandlerCtx) -> bool {
        if event.count > 0 {
            self.last_xy = Some((event.x, event.y));
            self.draw_mode = None;
            ctx.set_active(true);
        } else {
            self.last_xy = None;
            ctx.set_active(false);
        }
        true
    }

    fn mouse_moved(&mut self, x: f32, y: f32, ctx: &mut HandlerCtx) {
        if let Some((x0, y0)) = self.last_xy {
            let pts = self.line_quantize(x0, y0, x, y);
            for ((x0, y0), (x1, y1)) in pts.iter().tuple_windows() {
                let grid_ix = WireGrid::unit_line_to_grid_ix(*x0, *y0, *x1, *y1);
                if self.draw_mode.is_none() {
                    self.draw_mode = Some(!self.grid.is_set(grid_ix));
                }
                self.grid.set(grid_ix, self.draw_mode.unwrap());
            }
            if pts.len() > 1 {
                ctx.invalidate();
            }
            self.last_xy = Some((x, y))
        }
    }
}

impl Patcher {
    pub fn new() -> Patcher {
        Patcher {
            size: (0.0, 0.0),
            grid_size: (20, 16),
            offset: (5.0, 5.0),
            scale: 20.0,

            grid: Default::default(),
            last_xy: None,
            draw_mode: None,
        }
    }

    pub fn ui(self, ctx: &mut UiInner) -> Id {
        ctx.add(self, &[])
    }

    // Not sure this will be used.
    /*
    fn xy_to_cell(&self, x: f32, y: f32) -> Option<(u16, u16)> {
        let u = (x - self.offset.0) / self.scale;
        let v = (y - self.offset.1) / self.scale;
        self.guard_point(u, v)
    }
    */

    // TODO: This is not necessarily the absolute perfect logic, but it should work.
    fn line_quantize(&self, x0: f32, y0: f32, x1: f32, y1: f32) -> Vec<(u16, u16)> {
        let mut u = (x0 - self.offset.0) / self.scale;
        let mut v = (y0 - self.offset.1) / self.scale;
        let u1 = (x1 - self.offset.0) / self.scale;
        let v1 = (y1 - self.offset.1) / self.scale;
        let du = u1 - u;
        let dv = v1 - v;
        let mut vec = Vec::new();
        vec.extend(self.guard_point(u, v));
        let mut last_u = u.floor();
        let mut last_v = v.floor();
        if du.abs() > dv.abs() {
            while u.floor() != u1.floor() {
                let new_u = if du > 0.0 {
                    u.floor() + 1.0
                } else {
                    u.ceil() - 1.0
                };
                if new_u.floor() != last_u {
                    vec.extend(self.guard_point(new_u, last_v));
                }
                v += (new_u - u) * dv / du;
                u = new_u;
                if v.floor() != last_v {
                    vec.extend(self.guard_point(u, v));
                }
                last_u = u.floor();
                last_v = v.floor();
            }
        } else {
            while v.floor() != v1.floor() {
                let new_v = if dv > 0.0 {
                    v.floor() + 1.0
                } else {
                    v.ceil() - 1.0
                };
                if new_v.floor() != last_v {
                    vec.extend(self.guard_point(last_u, new_v));
                }
                u += (new_v - v) * du / dv;
                v = new_v;
                if u.floor() != last_u {
                    vec.extend(self.guard_point(u, v));
                }
                last_u = u.floor();
                last_v = v.floor();
            }
        }
        if u1.floor() != last_u || v1.floor() != last_v {
            vec.extend(self.guard_point(u1, v1));

        }
        vec
    }

    fn guard_point(&self, u: f32, v: f32) -> Option<(u16, u16)> {
        let i = u.floor() as isize;
        let j = v.floor() as isize;
        if i >= 0 && j >= 0 && (i as usize) < self.grid_size.0 && (j as usize) < self.grid_size.1 {
            Some((i as u16, j as u16))
        } else {
            None
        }
    }
}

impl WireGrid {
    fn set(&mut self, grid_ix: (u16, u16, bool), val: bool) {
        if val {
            self.grid.insert(grid_ix);
        } else {
            self.grid.remove(&grid_ix);
        }
    }

    fn is_set(&self, grid_ix: (u16, u16, bool)) -> bool {
        self.grid.contains(&grid_ix)
    }

    fn unit_line_to_grid_ix(x0: u16, y0: u16, x1: u16, y1: u16) -> (u16, u16, bool) {
        if x1 == x0 + 1 {
            (x0, y0, false)
        } else if x0 == x1 + 1 {
            (x1, y0, false)
        } else if y1 == y0 + 1 {
            (x0, y0, true)
        } else if y0 == y1 + 1 {
            (x0, y1, true)
        } else {
            panic!("not a unit line, logic error");
        }
    }
}
