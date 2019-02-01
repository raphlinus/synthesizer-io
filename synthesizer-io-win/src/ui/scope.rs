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

//! Widget for oscilloscope display.

use std::any::Any;

use kurbo::Rect;

use piet::{ImageFormat, InterpolationMode, RenderContext};

use druid::{BoxConstraints, HandlerCtx, LayoutCtx, LayoutResult};
use druid::{Geometry, Id, PaintCtx, Ui, Widget};

use synthesize_scope as s;

pub struct Scope {
    // I might want to call the data structure ScopeBuf or some such,
    // too many name collisions :/
    s: s::Scope,
}

#[derive(Clone, Debug)]
pub enum ScopeCommand {
    Start,
    Samples(Vec<f32>),
}

impl Widget for Scope {
    fn paint(&mut self, paint_ctx: &mut PaintCtx, geom: &Geometry) {
        let rc = &mut *paint_ctx.render_ctx;
        let w = 640;
        let h = 480;
        let data = self.s.as_rgba();
        let b = rc.make_image(w, h, &data, ImageFormat::RgbaPremul).unwrap();
        let height = geom.size.1.min(0.75 * geom.size.0);
        let width = height * (1.0 / 0.75);
        let x0 = geom.pos.0;
        let y0 = geom.pos.1;
        rc.draw_image(
            &b,
            Rect::new(
                (x0 + geom.size.0 - width) as f64,
                y0 as f64,
                (x0 + geom.size.0) as f64,
                (y0 + height) as f64,
            ),
            InterpolationMode::Bilinear,
        );
    }

    fn layout(
        &mut self,
        bc: &BoxConstraints,
        _children: &[Id],
        _size: Option<(f32, f32)>,
        _ctx: &mut LayoutCtx,
    ) -> LayoutResult {
        let size = bc.constrain((100.0, 100.0));
        //self.size = size;
        LayoutResult::Size(size)
    }

    fn poke(&mut self, payload: &mut Any, ctx: &mut HandlerCtx) -> bool {
        if let Some(cmd) = payload.downcast_ref::<ScopeCommand>() {
            match cmd {
                ScopeCommand::Start => ctx.request_anim_frame(),
                ScopeCommand::Samples(samples) => self.s.provide_samples(&samples),
            }
            true
        } else {
            println!("downcast failed in scope");
            false
        }
    }

    fn anim_frame(&mut self, _interval: u64, ctx: &mut HandlerCtx) {
        ctx.send_event(());
        ctx.request_anim_frame();
    }
}

impl Scope {
    pub fn new() -> Scope {
        let s = s::Scope::new(640, 480);
        Scope { s }
    }

    pub fn ui(self, ui: &mut Ui) -> Id {
        let id = ui.add(self, &[]);
        ui.poke(id, &mut ScopeCommand::Start);
        id
    }

    fn draw_test_pattern(&mut self) {
        let mut xylast = None;
        // sinewave!
        for i in 0..1001 {
            let h = (i as f32) * 0.001;
            let x = 640.0 * h;
            let y = 240.0 + 200.0 * (h * 50.0).sin();
            if let Some((xlast, ylast)) = xylast {
                self.s.add_line(xlast, ylast, x, y, 1.0, 2.0);
            }
            xylast = Some((x, y));
        }
    }
}
