use std::cmp::Ordering::*;
use std::mem::{self, ManuallyDrop};
use std::sync::atomic::Ordering;

use crate::ConcurrentSet;
use crossbeam_epoch::{pin, Atomic, Guard, Owned, Shared};
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

            let curr_node = unsafe { self.curr.deref() };
            match key.cmp(&curr_node.data) {
                Less => return Ok(false),
                Equal => return Ok(true),
                Greater => {
                    let old_prev = std::mem::replace(&mut self.prev,unsafe { curr_node.next.read_lock() });
                    old_prev.finish();
                    self.curr = self.prev.load(Ordering::SeqCst, guard);
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
            result.unwrap().0
        } else {
            false
        }
    }

    fn insert(&self, key: T) -> bool {
        let guard = &pin();
        let mut cursor = self.head(guard);
        if let Ok(found) = cursor.find(&key, guard) {
            if found {
                return false;
            }
        }
        let mut new_node = Node::new(key, cursor.curr);
        loop {
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
                    new_node = e.new;
                }
            }
        }
    }

    fn remove(&self, key: &T) -> bool {
        let guard = &pin();
        let mut cursor = self.head(guard);
        if let Ok(found) = cursor.find(key, guard) {
            if found {
                let succ = unsafe { cursor.curr.deref() }
                    .next
                    .write_lock()
                    .load(Ordering::SeqCst, guard);
                if cursor
                    .prev
                    .compare_exchange(cursor.curr, succ, Ordering::SeqCst, Ordering::SeqCst, guard)
                    .is_ok()
                {
                    unsafe {
                        guard.defer_destroy(cursor.curr);
                    }
                    cursor.prev.finish();
                    return true;
                }
            }
        }
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

impl<'g, T> Iterator for Iter<'g, T> {
    type Item = Result<&'g T, ()>;

    fn next(&mut self) -> Option<Self::Item> {
        let curr_node_ref = unsafe {self.cursor.curr.as_ref()}?;
        
        let result = unsafe {
            curr_node_ref.next.read(|next_atomic| {
                let next = next_atomic.load(Ordering::SeqCst, self.guard);
                self.cursor.curr = next;
            })
        };

        match result {
            Some(_) => {
                Some(Ok(&curr_node_ref.data))
            },
            None => {
                Some(Err(()))
            }
        }
    }
}

impl<T> Drop for OptimisticFineGrainedListSet<T> {
    fn drop(&mut self) {
        let guard = &pin();
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
