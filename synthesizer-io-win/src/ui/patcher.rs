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

use itertools::Itertools;

use direct2d::RenderTarget;
use direct2d::brush::SolidColorBrush;
use direct2d::enums::{AntialiasMode, CapStyle};
use direct2d::stroke_style::{StrokeStyle, StrokeStyleBuilder};

use xi_win_ui::{BoxConstraints, Geometry, LayoutResult, UiInner};
use xi_win_ui::{Id, HandlerCtx, LayoutCtx, PaintCtx};
use xi_win_ui::MouseEvent;
use xi_win_ui::widget::{MouseButton, Widget};

use grid::{Delta, ModuleGrid, ModuleInstance, ModuleSpec, WireDelta, WireGrid};

pub struct Patcher {
    size: (f32, f32),
    grid_size: (usize, usize),
    offset: (f32, f32),
    scale: f32,

    mode: PatcherMode,

    grid: WireGrid,
    last_xy: Option<(f32, f32)>,
    draw_mode: Option<bool>,

    modules: ModuleGrid,
    mod_hover: Option<ModuleInstance>,
}

enum PatcherMode {
    Wire,
    Module,
}

struct PaintResources {
    grid_color: SolidColorBrush,
    wire_color: SolidColorBrush,
    hover_ok: SolidColorBrush,
    hover_bad: SolidColorBrush,
    module_color: SolidColorBrush,
    rounded: StrokeStyle,
}

impl PaintResources {
    fn create(paint_ctx: &mut PaintCtx) -> PaintResources {
        // PaintCtx API is awkward, can't borrow d2d_factory while render_target
        // is borrowed. This works but should be improved (by having state splitting).
        let rounded = StrokeStyleBuilder::new(paint_ctx.d2d_factory())
            .with_start_cap(CapStyle::Round)
            .with_end_cap(CapStyle::Round)
            .build().unwrap();
        let rt = paint_ctx.render_target();
        let grid_color = SolidColorBrush::create(rt).with_color(0x405070).build().unwrap();
        let wire_color = SolidColorBrush::create(rt).with_color(0x808080).build().unwrap();
        let hover_ok = SolidColorBrush::create(rt).with_color((0x00c000, 0.5)).build().unwrap();
        let hover_bad = SolidColorBrush::create(rt).with_color((0xc00000, 0.5)).build().unwrap();
        let module_color = SolidColorBrush::create(rt).with_color(0xc0c0c0).build().unwrap();
        PaintResources { grid_color, wire_color, hover_ok, hover_bad, module_color, rounded }
    }
}

impl Widget for Patcher {
    fn paint(&mut self, paint_ctx: &mut PaintCtx, geom: &Geometry) {
        // TODO: retain these resources where possible
        let resources = PaintResources::create(paint_ctx);
        let rt = paint_ctx.render_target();
        self.paint_wiregrid(rt, &resources, geom);
        self.paint_modules(rt, &resources, geom);
        rt.pop_axis_aligned_clip();
    }

    fn layout(&mut self, bc: &BoxConstraints, _children: &[Id], _size: Option<(f32, f32)>,
        _ctx: &mut LayoutCtx) -> LayoutResult
    {
        let size = bc.constrain((100.0, 100.0));
        self.size = size;
        LayoutResult::Size(size)
    }

    fn mouse(&mut self, event: &MouseEvent, ctx: &mut HandlerCtx) -> bool {
        // Middle mouse button cycles through modes
        if event.which == MouseButton::Middle {
            if event.count > 0 {
                let new_mode = match self.mode {
                    PatcherMode::Wire => PatcherMode::Module,
                    PatcherMode::Module => PatcherMode::Wire,
                };
                self.mode = new_mode;
                self.update_hover(None, ctx);
            }
            return true;
        }
        match self.mode {
            PatcherMode::Wire => {
                if event.count > 0 {
                    self.last_xy = Some((event.x, event.y));
                    self.draw_mode = None;
                    ctx.set_active(true);
                } else {
                    self.last_xy = None;
                    ctx.set_active(false);
                }
            }
            PatcherMode::Module => {
                if let Some(mut inst) = self.mod_hover.take() {
                    // TODO: reduce dupl
                    let xc = event.x - 0.5 * self.scale * (inst.spec.size.0 as f32 - 1.0);
                    let yc = event.y - 0.5 * self.scale * (inst.spec.size.1 as f32 - 1.0);
                    if let Some(loc) = self.xy_to_cell(xc, yc) {
                        inst.loc = loc;
                        if self.is_module_ok(&inst) {
                            let delta = vec![Delta::Module(inst)];
                            self.apply_and_send_delta(delta, ctx);
                            /*
                            println!("placing {} at {:?}", inst.spec.name, inst.loc);
                            self.modules.add(inst);
                            ctx.send_event(vec![Delta::Module]);
                            ctx.invalidate();
                            */
                        }
                    }
                }
            }
        }
        true
    }

    fn mouse_moved(&mut self, x: f32, y: f32, ctx: &mut HandlerCtx) {
        match self.mode {
            PatcherMode::Wire => {
                if let Some((x0, y0)) = self.last_xy {
                    let mut delta = Vec::new();
                    let pts = self.line_quantize(x0, y0, x, y);
                    for ((x0, y0), (x1, y1)) in pts.iter().tuple_windows() {
                        let grid_ix = WireGrid::unit_line_to_grid_ix(*x0, *y0, *x1, *y1);
                        if self.draw_mode.is_none() {
                            self.draw_mode = Some(!self.grid.is_set(grid_ix));
                        }
                        let val = self.draw_mode.unwrap();
                        delta.push(Delta::Wire(WireDelta { grid_ix, val }));
                    }
                    self.apply_and_send_delta(delta, ctx);
                    self.last_xy = Some((x, y))
                }
            }
            PatcherMode::Module => {
                // could reduce the allocation here, but no biggie
                let spec = if let Some(ref h) = self.mod_hover {
                    h.spec.clone()
                } else {
                    ModuleSpec {
                        size: (2, 2),
                        name: "555".into(),
                    }
                };
                let xc = x - 0.5 * self.scale * (spec.size.0 as f32 - 1.0);
                let yc = y - 0.5 * self.scale * (spec.size.1 as f32 - 1.0);
                let instance = self.xy_to_cell(xc, yc).map(|loc| {
                    ModuleInstance { loc, spec }
                });
                self.update_hover(instance, ctx);
            }
        }
    }

    fn on_hot_changed(&mut self, hot: bool, ctx: &mut HandlerCtx) {
        if !hot {
            self.update_hover(None, ctx);
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

            mode: PatcherMode::Module,

            grid: Default::default(),
            last_xy: None,
            draw_mode: None,

            modules: Default::default(),
            mod_hover: None,
        }
    }

    pub fn ui(self, ctx: &mut UiInner) -> Id {
        ctx.add(self, &[])
    }


    // We actually have RT = GenericRenderTarget in the current impl. This could be a simple
    // type alias instead of parameterization. I'm wondering whether there might be a better
    // way to do this, but of course ultimately all this stuff should be wrapped to make it
    // less platform specific.
    fn paint_wiregrid<RT>(&mut self, rt: &mut RT, resources: &PaintResources, geom: &Geometry)
        where RT: RenderTarget
    {
        rt.push_axis_aligned_clip(geom, AntialiasMode::Aliased);
        let x0 = geom.pos.0 + self.offset.0;
        let y0 = geom.pos.1 + self.offset.1;
        for i in 0..(self.grid_size.0 + 1) {
            rt.draw_line((x0 + self.scale * (i as f32), y0),
                (x0 + self.scale * (i as f32), y0 + self.scale * (self.grid_size.1 as f32)),
                &resources.grid_color, 1.0, None);
        }
        for i in 0..(self.grid_size.1 + 1) {
            rt.draw_line((x0, y0 + self.scale * (i as f32)),
                (x0 + self.scale * (self.grid_size.0 as f32), y0 + self.scale * (i as f32)),
                &resources.grid_color, 1.0, None);
        }
        for (i, j, vert) in self.grid.iter() {
            let x = x0 + (*i as f32 + 0.5) * self.scale;
            let y = y0 + (*j as f32 + 0.5) * self.scale;
            let (x1, y1) = if *vert { (x, y + self.scale) } else { (x + self.scale, y) };
            rt.draw_line((x, y), (x1, y1), &resources.wire_color, 3.0,
                Some(&resources.rounded));
        }
    }

    fn paint_modules<RT>(&mut self, rt: &mut RT, resources: &PaintResources, geom: &Geometry)
        where RT: RenderTarget
    {
        for inst in self.modules.iter() {
            self.paint_module(rt, resources, geom, inst);
        }
        if let Some(ref inst) = self.mod_hover {
            let (i, j) = inst.loc;
            let (w, h) = inst.spec.size;
            let x0 = geom.pos.0 + self.offset.0;
            let y0 = geom.pos.1 + self.offset.1;
            let color = if self.is_module_ok(inst) {
                &resources.hover_ok
            } else {
                &resources.hover_bad
            };
            rt.fill_rectangle((x0 + (i as f32) * self.scale, y0 + (j as f32) * self.scale,
                    x0 + ((i + w) as f32) * self.scale, y0 + ((j + h) as f32) * self.scale),
                color);
        }
    }

    fn paint_module<RT>(&self, rt: &mut RT, resources: &PaintResources, geom: &Geometry,
        inst: &ModuleInstance) where RT: RenderTarget
    {
        let x0 = geom.pos.0 + self.offset.0;
        let y0 = geom.pos.1 + self.offset.1;
        let inset = 0.1;
        rt.fill_rectangle((x0 + (inst.loc.0 as f32 + inset) * self.scale,
                y0 + (inst.loc.1 as f32 + inset) * self.scale,
                x0 + ((inst.loc.0 + inst.spec.size.0) as f32 - inset) * self.scale,
                y0 + ((inst.loc.1 + inst.spec.size.1) as f32 - inset) * self.scale),
            &resources.module_color);
        for j in 0..inst.spec.size.1 {
            let xl = x0 + (inst.loc.0 as f32 + inset) * self.scale;
            let xr = x0 + ((inst.loc.0 + inst.spec.size.0) as f32 - inset) * self.scale;
            let y = y0 + ((inst.loc.1 + j) as f32 + 0.5) * self.scale;
            let width = 2.0;
            rt.draw_line((xl, y), (xl - (0.5 + inset) * self.scale, y),
                &resources.module_color, width, None);
            rt.draw_line((xr, y), (xr + (0.5 + inset) * self.scale, y),
                &resources.module_color, width, None);
        }
    }

    fn xy_to_cell(&self, x: f32, y: f32) -> Option<(u16, u16)> {
        let u = (x - self.offset.0) / self.scale;
        let v = (y - self.offset.1) / self.scale;
        self.guard_point(u, v)
    }

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

    fn update_hover(&mut self, hover: Option<ModuleInstance>, ctx: &mut HandlerCtx) {
        if self.mod_hover != hover {
            self.mod_hover = hover;
            ctx.invalidate();
        }
    }

    fn is_module_ok(&self, inst: &ModuleInstance) -> bool {
        !self.modules.is_conflict(inst)
    }

    fn apply_and_send_delta(&mut self, delta: Vec<Delta>, ctx: &mut HandlerCtx) {
        if !delta.is_empty() {
            self.apply_delta(&delta);
            ctx.send_event(delta);
            ctx.invalidate();
        }
    }

    fn apply_delta(&mut self, delta: &[Delta]) {
        for d in delta {
            match d {
                Delta::Wire(WireDelta { grid_ix, val }) => {
                    self.grid.set(*grid_ix, *val);
                }
                Delta::Module(inst) => {
                    self.modules.add(inst.clone());
                }
            }
        }
    }
}

