// Copyright 2017 The Synthesizer IO Authors.
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

//! A lock-free queue suitable for real-time audio threads.

use std::sync::atomic::{AtomicPtr, Ordering};
use std::sync::atomic::Ordering::{Relaxed, Release};
use std::sync::Arc;
use std::thread;
use std::ptr;
use std::ptr::NonNull;
use std::ops::{Deref, DerefMut};
use std::marker::PhantomData;
use std::time;

// The implementation is a fairly straightforward Treiber stack.

struct Node<T> {
    payload: T,
    child: Option<NonNull<Node<T>>>,
}

impl<T> Node<T> {
    // reverse singly-linked list in place
    unsafe fn reverse(mut p: Option<NonNull<Node<T>>>) -> Option<NonNull<Node<T>>> {
        let mut q = None;
        while let Some(mut element) = p {
            p = element.as_ref().child;
            element.as_mut().child = q;
            q = Some(element);
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
    ptr: NonNull<Node<T>>,
}
// TODO: it would be great to disable drop

unsafe impl<T: Send> Send for Item<T> {}

impl<T> Item<T> {
    /// Create an `Item` for the given value. This function allocates and is
    /// very similar to `Box::new()`.
    pub fn make_item(payload: T) -> Item<T> {
        let ptr = Box::into_raw(Box::new(Node {
            payload: payload,
            child: None,
        }));
        // TODO: use Box::into_raw_non_null when it stabilizes
        unsafe {
            Item { ptr: NonNull::new_unchecked(ptr) }
        }
    }
}

impl<T> Deref for Item<T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { &self.ptr.as_ref().payload }
    }
}

impl<T> DerefMut for Item<T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut self.ptr.as_mut().payload }
    }
}

/// A lock-free queue specialized for audio applications.
///
/// The special super-power of this implementation is that it allows all allocation
/// and deallocation to be done on _one_ side (either producer or consumer) of the
/// queue. Thus, an audio processing thread can be rigorously nonblocking even as
/// it receives dynamically allocated messages. Instead of dropping the messages, it
/// sends them through a return-path queue.
///
/// Following Dmitry Vyukov's
/// [classification](http://www.1024cores.net/home/lock-free-algorithms/queues),
/// this queue is MPSC, linked-list-based, intrusive, unbounded, does not require
/// GC, and does not have support for message priorities. It provides per-producer
/// FIFO, has lockfree producers and waitfree consumers.
///
/// Note that Vyukov's
/// [intrusive mpsc queue](http://www.1024cores.net/home/lock-free-algorithms/queues/intrusive-mpsc-node-based-queue)
/// might have better performance due to not needing to reverse. See
/// [this thread](https://groups.google.com/forum/#!topic/lock-free/i0eE2-A7eIA) for discussion
/// of performance and an argument why this design is in fact multi-producer safe.

pub struct Queue<T> {
    head: AtomicPtr<Node<T>>,
}

// implement Send (so queue can be transferred into worker thread)
unsafe impl<T: Send> Send for Sender<T> {}
// implement Sync, as queue is multi-producer safe.
unsafe impl<T: Sync> Sync for Sender<T> {}

/// The sender endpoint for a lock-free queue.
pub struct Sender<T> {
    queue: Arc<Queue<T>>,
    // TODO: is this phantom data necessary?
    _marker: PhantomData<*const T>,
}

unsafe impl<T: Send> Send for Receiver<T> {}
// Note: could implement Sync and Clone, but value is marginal.

/// The receiver endpoint for a lock-free queue.
pub struct Receiver<T> {
    queue: Arc<Queue<T>>,
    _marker: PhantomData<*const T>,
}

impl<T: Send + 'static> Clone for Sender<T> {
    fn clone(&self) -> Sender<T> {
        Sender {
            queue: self.queue.clone(),
            _marker: Default::default(),
        }
    }
}

impl<T: Send + 'static> Sender<T> {
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

impl<T: Send + 'static> Receiver<T> {
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

impl<T: Send + 'static> Queue<T> {
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

    fn push_raw(&self, mut n: NonNull<Node<T>>) {
        let mut old_ptr = self.head.load(Relaxed);
        loop {
            unsafe { n.as_mut().child = NonNull::new(old_ptr); }
            match self.head.compare_exchange_weak(old_ptr, n.as_ptr(), Release, Relaxed) {
                Ok(_) => break,
                Err(old) => old_ptr = old,
            }
        }
    }

    // yields linked list in reverse order as sent
    fn pop_all(&self) -> Option<NonNull<Node<T>>> {
        NonNull::new(self.head.swap(ptr::null_mut(), Ordering::Acquire))
    }
}

/// An iterator yielding an `Item` for each value dequeued by a `recv_items` call.
pub struct QueueItemIter<T: Send + 'static>(Option<NonNull<Node<T>>>);

impl<T: Send + 'static> Iterator for QueueItemIter<T> {
    type Item = Item<T>;
    fn next(&mut self) -> Option<Item<T>> {
        unsafe {
            self.0.map(|ptr| {
                self.0 = ptr.as_ref().child;
                Item { ptr }
            })
        }
    }
}

/// An iterator yielding the values dequeued by a `recv` call.
pub struct QueueMoveIter<T: Send + 'static>(Option<NonNull<Node<T>>>);

impl<T: Send + 'static> Iterator for QueueMoveIter<T> {
    type Item = T;
    fn next(&mut self) -> Option<T> {
        unsafe {
            self.0.map(|ptr| {
                let result = Box::from_raw(ptr.as_ptr());
                self.0 = result.child;
                result.payload
            })
        }
    }
}

impl<T: Send + 'static> Drop for QueueMoveIter<T> {
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
