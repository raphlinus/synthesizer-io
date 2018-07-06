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

//! A lock-free queue suitable for real-time audio threads

use std::sync::atomic::{AtomicPtr, Ordering};
use std::sync::atomic::Ordering::{Relaxed, Release};
use std::sync::Arc;
use std::thread;
use std::ptr;
use std::ops::{Deref, DerefMut};
use std::marker::PhantomData;
use std::time;

// The implementation is a fairly straightforward Treiber stack.

struct Node<T> {
    payload: T,
    child: *mut Node<T>,
}

impl<T> Node<T> {
    // reverse singly-linked list in place
    unsafe fn reverse(mut p: *mut Node<T>) -> *mut Node<T> {
        let mut q = ptr::null_mut();
        while !p.is_null() {
            let element = p;
            p = (*element).child;
            (*element).child = q;
            q = element;
        }
        q
    }
}

/// A structure that owns a value. It acts a lot like `Box`, but has the
/// special property that it can be sent back over a channel with zero
/// allocation.
///
/// Note: in the current implementation, dropping an `Item` just leaks the
/// storage.
pub struct Item<T> {
    ptr: *mut Node<T>,
    // TODO: can use NonZero once that stabilizes, for optimization
    // TODO: does this need a PhantomData marker?
}
// TODO: it would be great to disable drop

impl<T> Item<T> {
    pub fn make_item(payload: T) -> Item<T> {
        let ptr = Box::into_raw(Box::new(Node {
            payload: payload,
            child: ptr::null_mut(),
        }));
        Item { ptr: ptr }
    }
}

impl<T> Deref for Item<T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { &(*self.ptr).payload }
    }
}

impl<T> DerefMut for Item<T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut (*self.ptr).payload }
    }
}

pub struct Queue<T> {
    head: AtomicPtr<Node<T>>,
}

// implement send (so queue can be transferred into worker thread)
// but not sync (to enforce spsc, which avoids ABA)
unsafe impl<T> Send for Sender<T> {}
pub struct Sender<T> {
    queue: Arc<Queue<T>>,
    _marker: PhantomData<*const T>,
}

unsafe impl<T> Send for Receiver<T> {}
pub struct Receiver<T> {
    queue: Arc<Queue<T>>,
    _marker: PhantomData<*const T>,
}

impl<T: 'static> Sender<T> {
    /// Enqueue a value into the queue. Note: this method allocates.
    pub fn send(&self, payload: T) {
        self.queue.send(payload);
    }

    /// Enqueue a value held in an `Item` into the queue. This method does
    /// not allocate.
    pub fn send_item(&self, item: Item<T>) {
        self.queue.send_item(item);
    }
}

impl<T: 'static> Receiver<T> {
    /// Dequeue all of the values waiting in the queue, and return an iterator
    /// that transfers ownership of those values. Note: the iterator
    /// will deallocate.
    pub fn recv(&self) -> QueueMoveIter<T> {
        self.queue.recv()
    }

    /// Dequeue all of the values waiting in the queue, and return an iterator
    /// that transfers ownership of those values into `Item` structs.
    /// Neither this method nor the iterator do any allocation.
    pub fn recv_items(&self) -> QueueItemIter<T> {
        self.queue.recv_items()
    }
}

impl<T: 'static> Queue<T> {
    /// Create a new queue, and return endpoints for sending and receiving.
    pub fn new() -> (Sender<T>, Receiver<T>) {
        let queue = Arc::new(Queue {
            head: AtomicPtr::new(ptr::null_mut()),
        });
        (Sender {
            queue: queue.clone(),
            _marker: Default::default(),
        },
        Receiver {
            queue: queue,
            _marker: Default::default(),
        })
    }

    fn send(&self, payload: T) {
        self.send_item(Item::make_item(payload));
    }

    fn recv(&self) -> QueueMoveIter<T> {
        unsafe { QueueMoveIter(Node::reverse(self.pop_all())) }
    }

    fn send_item(&self, item: Item<T>) {
        self.push_raw(item.ptr);
    }

    fn recv_items(&self) -> QueueItemIter<T> {
        unsafe { QueueItemIter(Node::reverse(self.pop_all())) }
    }

    fn push_raw(&self, n: *mut Node<T>) {
        let mut old_ptr = self.head.load(Relaxed);
        loop {
            unsafe { (*n).child = old_ptr; }
            match self.head.compare_exchange_weak(old_ptr, n, Release, Relaxed) {
                Ok(_) => break,
                Err(old) => old_ptr = old,
            }
        }
    }

    // yields linked list in reverse order as sent
    fn pop_all(&self) -> *mut Node<T> {
        self.head.swap(ptr::null_mut(), Ordering::Acquire)
    }
}

/// An iterator yielding an `Item` for each value dequeued by a `recv_items` call.
pub struct QueueItemIter<T: 'static>(*mut Node<T>);

impl<T> Iterator for QueueItemIter<T> {
    type Item = Item<T>;
    fn next(&mut self) -> Option<Item<T>> {
        unsafe {
            let result = self.0.as_mut();
            if !self.0.is_null() {
                self.0 = (*self.0).child;
            }
            result.map(|ptr| Item{ ptr: ptr })
        }
    }
}

/// An iterator yielding the values dequeued by a `recv` call.
pub struct QueueMoveIter<T: 'static>(*mut Node<T>);

impl<T> Iterator for QueueMoveIter<T> {
    type Item = T;
    fn next(&mut self) -> Option<T> {
        unsafe {
            if self.0.is_null() {
                None
            } else {
                let result = *Box::from_raw(self.0);
                self.0 = result.child;
                Some(result.payload)
            }
        }
    }
}

impl<T: 'static> Drop for QueueMoveIter<T> {
    fn drop(&mut self) {
        self.all(|_| true);
    }
}

// Use case code below, to be worked in a separate module. Would also be
// a good basis for a test.

struct Worker {
    to_worker: Receiver<String>,
    from_worker: Sender<String>,
}

impl Worker {
    fn work(&mut self) {
        let mut things = Vec::new();

        let start = time::Instant::now();
        loop {
            for node in self.to_worker.recv_items() {
                things.push(node);
            }
            if things.len() >= 1000 {
                break;
            }
            thread::sleep(time::Duration::new(0, 5000));
        }
        let elapsed = start.elapsed();
        for thing in things {
            self.from_worker.send_item(thing);
        }
        println!("#total time: {:?}", elapsed);
    }
}

pub fn try_queue() {
    let (tx, to_worker) = Queue::new();
    let (from_worker, rx) = Queue::new();
    let mut worker = Worker {
        to_worker: to_worker,
        from_worker: from_worker,
    };
    let child = thread::spawn(move || worker.work());
    thread::sleep(time::Duration::from_millis(1));
    for i in 0..1000 {
        tx.send(i.to_string());
        //thread::sleep(time::Duration::new(0, 1000));
    }
    let mut n_recv = 0;
    loop {
        for s in rx.recv() {
            println!("{}", s);
            n_recv += 1;
        }
        if n_recv == 1000 {
            break;
        }
    }
    let _ = child.join();
    //println!("done");
}
