// Copyright 2017 Google Inc. All rights reserved.
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

//! A collection of audio processing modules.

mod sum;
mod buzz;
mod sin;
mod saw;
mod biquad;
mod const_ctrl;
mod smooth_ctrl;
mod note_pitch;
mod adsr;
mod gain;

pub use self::sum::Sum;
pub use self::buzz::Buzz;
pub use self::sin::Sin;
pub use self::saw::Saw;
pub use self::biquad::Biquad;
pub use self::const_ctrl::ConstCtrl;
pub use self::smooth_ctrl::SmoothCtrl;
pub use self::note_pitch::NotePitch;
pub use self::adsr::Adsr;
pub use self::gain::Gain;
