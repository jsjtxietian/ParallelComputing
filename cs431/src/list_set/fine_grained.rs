use std::cmp::Ordering;
use std::cmp::Ordering::*;
use std::fmt::Display;
use std::mem;
use std::ptr;
use std::sync::{Mutex, MutexGuard};

use crate::ConcurrentSet;

#[derive(Debug)]
struct Node<T> {
    data: T,
    next: Mutex<*mut Node<T>>,
}

/// Concurrent sorted singly linked list using fine-grained lock-coupling.
#[derive(Debug)]
pub struct FineGrainedListSet<T> {
    head: Mutex<*mut Node<T>>,
}

unsafe impl<T: Send> Send for FineGrainedListSet<T> {}
unsafe impl<T: Send> Sync for FineGrainedListSet<T> {}

/// Reference to the `next` field of previous node which points to the current node.
///
/// For example, given the following linked list:
///
/// ```text
/// head -> 1 -> 2 -> 3 -> null
/// ```
///
/// If `cursor` is currently at node 2, then `cursor.0` should be the `MutexGuard` obtained from the
/// `next` of node 1. In particular, `cursor.0.as_ref().unwrap()` creates a shared reference to node
/// 2.
struct Cursor<'l, T>(MutexGuard<'l, *mut Node<T>>);

impl<T> Node<T> {
    fn new(data: T, next: *mut Self) -> *mut Self {
        Box::into_raw(Box::new(Self {
            data,
            next: Mutex::new(next),
        }))
    }
}

impl<T: Ord> Cursor<'_, T> {
    /// Moves the cursor to the position of key in the sorted list.
    /// Returns whether the value was found.
    fn find(&mut self, key: &T) -> bool {
        loop {
            let current_ptr = *self.0;

            if current_ptr.is_null() {
                return false;
            }

            let current_node = unsafe { &*current_ptr };

            match current_node.data.cmp(key) {
                Ordering::Greater => return false,
                Ordering::Less => self.0 = current_node.next.lock().unwrap(),

                Ordering::Equal => return true,
            }
        }
    }
}

impl<T> FineGrainedListSet<T> {
    /// Creates a new list.
    pub fn new() -> Self {
        Self {
            head: Mutex::new(ptr::null_mut()),
        }
    }
}

impl<T: Ord> FineGrainedListSet<T> {
    fn find(&self, key: &T) -> (bool, Cursor<'_, T>) {
        let mut cursor = Cursor(self.iter().cursor);
        (cursor.find(key), cursor)
    }
}

impl<T: Ord + Display> ConcurrentSet<T> for FineGrainedListSet<T> {
    fn contains(&self, key: &T) -> bool {
        self.find(key).0
    }

    fn insert(&self, key: T) -> bool {
        let mut prev_guard = self.head.lock().unwrap();
        loop {
            let current_ptr = *prev_guard;

            if current_ptr.is_null() {
                let node = Node::new(key, ptr::null_mut());
                *prev_guard = node;
                return true;
            }

            let current_node = unsafe { &*current_ptr };
            match key.cmp(&current_node.data) {
                Ordering::Greater => prev_guard = current_node.next.lock().unwrap(),
                Ordering::Less => {
                    let node = Node::new(key, current_ptr);
                    *prev_guard = node;
                    return true;
                }
                Ordering::Equal => return false,
            }
        }
    }

    fn remove(&self, key: &T) -> bool {
        let mut prev_guard = self.head.lock().unwrap();
        loop {
            let current_ptr = *prev_guard;

            if current_ptr.is_null() {
                return false;
            }

            let current_node = unsafe { &*current_ptr };
            match key.cmp(&current_node.data) {
                Ordering::Greater => {
                    prev_guard = current_node.next.lock().unwrap();
                }
                Ordering::Less => {
                    return false;
                }
                Ordering::Equal => {
                    let mut next_guard = current_node.next.lock().unwrap();
                    let new_next_node = *next_guard;
                    *prev_guard = new_next_node;
                    unsafe {
                        let _ = Box::from_raw(current_ptr);
                    }
                    return true;
                }
            }
        }
    }
}

#[derive(Debug)]
pub struct Iter<'l, T> {
    cursor: MutexGuard<'l, *mut Node<T>>,
}

impl<T> FineGrainedListSet<T> {
    /// An iterator visiting all elements.
    pub fn iter(&self) -> Iter<'_, T> {
        Iter {
            cursor: self.head.lock().unwrap(),
        }
    }
}

impl<'l, T> Iterator for Iter<'l, T> {
    type Item = &'l T;

    fn next(&mut self) -> Option<Self::Item> {
        let node_ptr = *(self.cursor);
        if node_ptr.is_null() {
            None
        } else {
            let result = Some(unsafe { &(*node_ptr).data });
            self.cursor = unsafe { (*node_ptr).next.lock().unwrap() };
            result
        }
    }
}

impl<T> Drop for FineGrainedListSet<T> {
    fn drop(&mut self) {
        let mut current_ptr = *self.head.lock().unwrap();
        while !current_ptr.is_null() {
            let current = unsafe { Box::from_raw(current_ptr) };
            current_ptr = *current.next.lock().unwrap();
        }
    }
}

impl<T> Default for FineGrainedListSet<T> {
    fn default() -> Self {
        Self::new()
    }
}
