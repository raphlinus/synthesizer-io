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

//! Super-simple allocator for id's.

/// A simple allocator for reusable unique id's, attempting to keep the
/// returned values small.
pub struct IdAllocator {
    // All id's in `free` are free.
    free: Vec<usize>,

    // All id's greater than or equal to `highwater` are free
    highwater: usize,
}

impl IdAllocator {
    /// Create a new allocator.
    pub fn new() -> IdAllocator {
        IdAllocator {
            free: Vec::new(),
            highwater: 0,
        }
    }

    /// Allocate a fresh id.
    pub fn alloc(&mut self) -> usize {
        if let Some(id) = self.free.pop() {
            id
        } else {
            let id = self.highwater;
            self.highwater += 1;
            id
        }
    }

    /// Free the id so it can be reused.
    pub fn free(&mut self, id: usize) {
        if id == self.highwater - 1 {
            self.highwater = id;
        } else {
            self.free.push(id);
        }
    }

    /// Reserve an id, preventing it from being issued.
    pub fn reserve(&mut self, id: usize) {
        if id == self.highwater {
            self.highwater += 1;
        } else {
            if let Some(pos) = self.free.iter().position(|x| *x == id) {
                self.free.remove(pos);
            } else {
                panic!("Attempting to reserve {}, already allocated", id);
            }
        }
    }
}
