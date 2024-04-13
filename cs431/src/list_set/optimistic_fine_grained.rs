use std::cmp::Ordering::*;
use std::f32::consts::E;
use std::fmt::Debug;
use std::mem::{self, ManuallyDrop};
use std::sync::atomic::Ordering;

use crate::ConcurrentSet;
use crossbeam_epoch::{pin, unprotected, Atomic, Guard, Owned, Shared};
use cs431::lock::seqlock::{ReadGuard, SeqLock};

#[derive(Debug)]
struct Node<T> {
    data: T,
    next: SeqLock<Atomic<Node<T>>>,
}

/// Concurrent sorted singly linked list using fine-grained optimistic locking.
#[derive(Debug)]
pub struct OptimisticFineGrainedListSet<T> {
    head: SeqLock<Atomic<Node<T>>>,
}

unsafe impl<T: Send> Send for OptimisticFineGrainedListSet<T> {}
unsafe impl<T: Send> Sync for OptimisticFineGrainedListSet<T> {}

#[derive(Debug)]
struct Cursor<'g, T> {
    // Reference to the `next` field of previous node which points to the current node.
    prev: ReadGuard<'g, Atomic<Node<T>>>,
    curr: Shared<'g, Node<T>>,
}

impl<T> Node<T> {
    fn new(data: T, next: Shared<'_, Self>) -> Owned<Self> {
        Owned::new(Self {
            data,
            next: SeqLock::new(next.into()),
        })
    }
}

impl<'g, T: Ord> Cursor<'g, T> {
    /// Moves the cursor to the position of key in the sorted list.
    /// Returns whether the value was found.
    ///
    /// Return `Err(())` if the cursor cannot move.
    fn find(&mut self, key: &T, guard: &'g Guard) -> Result<bool, ()> {
        loop {
            if self.curr.is_null() {
                return Ok(false);
            }

            let curr_node = unsafe { self.curr.as_ref() };
            match curr_node {
                Some(node) => match key.cmp(&node.data) {
                    Less => return Ok(false),
                    Equal => return Ok(true),
                    Greater => {
                        let prev_read_guard = unsafe { node.next.read_lock() };
                        if !prev_read_guard.validate() {
                            prev_read_guard.finish();
                            return Err(());
                        }
                        let old_prev = std::mem::replace(&mut self.prev, prev_read_guard);
                        old_prev.finish();
                        self.curr = self.prev.load(Ordering::SeqCst, guard);
                    }
                },
                None => {
                    return Err(());
                }
            }
        }
    }
}

impl<T> OptimisticFineGrainedListSet<T> {
    /// Creates a new list.
    pub fn new() -> Self {
        Self {
            head: SeqLock::new(Atomic::null()),
        }
    }

    fn head<'g>(&'g self, guard: &'g Guard) -> Cursor<'g, T> {
        let prev = unsafe { self.head.read_lock() };
        let curr = prev.load(Ordering::SeqCst, guard);
        Cursor { prev, curr }
    }
}

impl<T: Ord> OptimisticFineGrainedListSet<T> {
    fn find<'g>(&'g self, key: &T, guard: &'g Guard) -> Result<(bool, Cursor<'g, T>), ()> {
        let mut cursor = self.head(guard);
        let found = cursor.find(key, guard).map_err(|_| false).unwrap_or(false);
        Ok((found, cursor))
    }
}

impl<T: Ord> ConcurrentSet<T> for OptimisticFineGrainedListSet<T> {
    fn contains(&self, key: &T) -> bool {
        let guard = &pin();
        let result = self.find(key, guard).map_err(|_| false);
        if result.is_ok() {
            let result = result.unwrap();
            result.1.prev.finish();
            result.0
        } else {
            false
        }
    }

    fn insert(&self, key: T) -> bool {
        let guard = &pin();
        let mut cursor = self.head(guard);
        if let Ok(found) = cursor.find(&key, guard) {
            if found {
                cursor.prev.finish();
                return false;
            }
        }
        let mut new_node = Node::new(key, cursor.curr);
        match cursor.prev.compare_exchange(
            cursor.curr,
            new_node,
            Ordering::SeqCst,
            Ordering::SeqCst,
            guard,
        ) {
            Ok(node) => {
                cursor.prev.finish();
                return true;
            }
            Err(e) => {
                cursor.prev.finish();
                return false;
            }
        }
    }

    fn remove(&self, key: &T) -> bool {
        let guard = &pin();
        let mut cursor = self.head(guard);
        if let Ok(found) = cursor.find(key, guard) {
            if found {
                let succ = unsafe { cursor.curr.as_ref() };
                if succ.is_none() {
                    return false;
                }
                let succ = succ
                    .unwrap()
                    .next
                    .write_lock()
                    .load(Ordering::SeqCst, guard);
                let prev = cursor.prev.upgrade();
                match prev {
                    Ok(p) => {
                        if p.compare_exchange(
                            cursor.curr,
                            succ,
                            Ordering::SeqCst,
                            Ordering::SeqCst,
                            guard,
                        )
                        .is_ok()
                        {
                            unsafe {
                                guard.defer_destroy(cursor.curr);
                            }
                            return true;
                        } else {
                            return false;
                        }
                    }
                    Err(_) => {
                        return false;
                    }
                }
            }
        }
        cursor.prev.finish();
        false
    }
}

#[derive(Debug)]
pub struct Iter<'g, T> {
    // Can be dropped without validation, because the only way to use cursor.curr is next().
    cursor: ManuallyDrop<Cursor<'g, T>>,
    guard: &'g Guard,
}

impl<T> OptimisticFineGrainedListSet<T> {
    /// An iterator visiting all elements. `next()` returns `Some(Err(()))` when validation fails.
    /// In that case, the user must restart the iteration.
    pub fn iter<'g>(&'g self, guard: &'g Guard) -> Iter<'_, T> {
        Iter {
            cursor: ManuallyDrop::new(self.head(guard)),
            guard,
        }
    }
}

impl<'g, T> Iterator for Iter<'g, T>
where
    T: Debug,
{
    type Item = Result<&'g T, ()>;

    fn next(&mut self) -> Option<Self::Item> {
        let curr_node = self.cursor.curr;
        let prev_node = &self.cursor.prev;

        if !prev_node.validate() {
            return Some(Err(()));
        }

        if curr_node.is_null() {
            let real_node = prev_node.load(Ordering::SeqCst, self.guard);
            if real_node.is_null() {
                return None;
            } else {
                return Some(Err(()));
            }
        }

        let value = unsafe {
            let curr_node = curr_node.as_ref();
            if curr_node.is_none() {
                return None;
            }
            let curr_node = curr_node.unwrap();
            curr_node.next.read(|next_atomic| {
                let old_prev = std::mem::replace(&mut self.cursor.prev, curr_node.next.read_lock());
                old_prev.finish();

                let data = &curr_node.data;

                let value = next_atomic.load(Ordering::SeqCst, self.guard);
                self.cursor.curr = value;

                data
            })
        };

        match value {
            Some(data) => Some(Ok(data)),
            None => Some(Err(())),
        }
    }
}

impl<T> Drop for OptimisticFineGrainedListSet<T> {
    fn drop(&mut self) {
        let guard = unsafe {unprotected() };
        let mut curr = self.head.write_lock().load(Ordering::SeqCst, guard);
        while !curr.is_null() {
            unsafe {
                guard.defer_destroy(curr);
            }
            curr = unsafe { curr.deref() }
                .next
                .write_lock()
                .load(Ordering::SeqCst, guard);
        }
    }
}

impl<T> Default for OptimisticFineGrainedListSet<T> {
    fn default() -> Self {
        Self::new()
    }
}
