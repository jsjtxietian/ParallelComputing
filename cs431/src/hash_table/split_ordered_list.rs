//! Split-ordered linked list.

use core::mem::{self, MaybeUninit};
use core::sync::atomic::{AtomicUsize, Ordering::*};
use crossbeam_epoch::{Guard, Owned, Shared};
use cs431::lockfree::list::{Cursor, List, Node};

use super::growable_array::GrowableArray;
use crate::ConcurrentMap;

/// Lock-free map from `usize` in range \[0, 2^63-1\] to `V`.
///
/// NOTE: We don't care about hashing in this homework for simplicity.
#[derive(Debug)]
pub struct SplitOrderedList<V> {
    /// Lock-free list sorted by recursive-split order.
    ///
    /// Use `MaybeUninit::uninit()` when creating sentinel nodes.
    list: List<usize, MaybeUninit<V>>,
    /// Array of pointers to the buckets.
    buckets: GrowableArray<Node<usize, MaybeUninit<V>>>,
    /// Number of buckets.
    size: AtomicUsize,
    /// Number of items.
    count: AtomicUsize,
}

impl<V> Default for SplitOrderedList<V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<V> SplitOrderedList<V> {
    /// `size` is doubled when `count > size * LOAD_FACTOR`.
    const LOAD_FACTOR: usize = 2;

    /// Creates a new split ordered list.
    pub fn new() -> Self {
        Self {
            list: List::new(),
            buckets: GrowableArray::new(),
            size: AtomicUsize::new(2),
            count: AtomicUsize::new(0),
        }
    }

    // todo, check the book
    /// Creates a cursor and moves it to the bucket for the given index.  If the bucket doesn't
    /// exist, recursively initializes the buckets.
    fn lookup_bucket<'s>(
        &'s self,
        index: usize,
        guard: &'s Guard,
    ) -> Cursor<'s, usize, MaybeUninit<V>> {
        let bucket = self.buckets.get(index, guard).load(SeqCst, &guard);
        if bucket.is_null() {
            self.init_buckets(index, guard);
        }
        // Cursor::from_raw and Shared::as_raw ?
        // make sure prev does not outrun curr ?
        Cursor::new(
            self.buckets.get(index, guard),
            self.buckets.get(index, guard).load(SeqCst, &guard),
        )
    }

    /// Moves the bucket cursor returned from `lookup_bucket` to the position of the given key.
    /// Returns `(size, found, cursor)`
    fn find<'s>(
        &'s self,
        key: &usize,
        guard: &'s Guard,
    ) -> (usize, bool, Cursor<'s, usize, MaybeUninit<V>>) {
        let bucket_size = self.size.load(SeqCst);
        let bucket_index = key % bucket_size;

        // loop {
        let mut cursor = self.lookup_bucket(bucket_index, guard);
        let result = cursor.find_harris(&self.regular_key(*key), guard);
        if result.is_ok() {
            return (bucket_size, result.unwrap(), cursor);
        } else {
            todo!();
        }
        // }
    }

    fn regular_key(&self, key: usize) -> usize {
        let mut result = key;
        result |= 1 << (usize::BITS - 1);

        result.reverse_bits()
    }

    fn dummy_key(&self, key: usize) -> usize {
        if key == 0 {
            0
        } else {
            key.reverse_bits()
        }
    }

    fn get_parent(&self, num: usize) -> usize {
        if num == 0 {
            return 0;
        }

        let mut mask = 1;
        while mask <= num {
            mask <<= 1;
        }
        mask >>= 1;

        num - mask
    }

    fn init_buckets(&self, bucket_index: usize, guard: &Guard) {
        let dummy = Owned::new(Node::new(
            self.dummy_key(bucket_index),
            MaybeUninit::<V>::uninit(),
        ));

        let mut cursor: Cursor<usize, MaybeUninit<V>>;
        if bucket_index == 0 {
            cursor = self.list.head(guard);
        } else {
            let parent_index = self.get_parent(bucket_index);
            cursor = self.lookup_bucket(parent_index, guard);
        }

        let key = self.dummy_key(bucket_index);
        let find_result = cursor.find_harris(&key, guard);

        if let Ok(found) = find_result {
            if !found {
                let insert_result = cursor.insert(dummy, guard);
                if insert_result.is_ok() {
                    match self.buckets.get(bucket_index, guard).compare_exchange(
                        Shared::null(),
                        cursor.curr(),
                        SeqCst,
                        SeqCst,
                        guard,
                    ) {
                        Ok(_) => {}
                        Err(_) => {
                            todo!()
                        }
                    }
                } else {
                    drop(insert_result.unwrap_err());
                }
            }
        }
    }

    fn assert_valid_key(key: usize) {
        assert!(key.leading_zeros() != 0);
    }
}

impl<V> ConcurrentMap<usize, V> for SplitOrderedList<V> {
    fn lookup<'a>(&'a self, key: &usize, guard: &'a Guard) -> Option<&'a V> {
        Self::assert_valid_key(*key);
        let (bucket_size, found, mut cursor) = self.find(&key, guard);
        if !found {
            return None;
        } else {
            unsafe {
                return Some(cursor.lookup().assume_init_ref());
            }
        }
    }

    fn insert(&self, key: usize, value: V, guard: &Guard) -> Result<(), V> {
        Self::assert_valid_key(key);

        let (bucket_size, found, mut cursor) = self.find(&key, guard);
        if found {
            return Err(value);
        }

        let new_node = Owned::new(Node::new(self.regular_key(key), MaybeUninit::new(value)));
        let result = cursor.insert(new_node, guard);
        if let Err(owned_node) = result {
            unsafe {
                return Err(owned_node.into_box().into_value().assume_init());
            }
        }

        let count = self.count.fetch_add(1, SeqCst) + 1;
        if count > bucket_size * SplitOrderedList::<V>::LOAD_FACTOR {
            let _ = self
                .size
                .compare_exchange(bucket_size, bucket_size * 2, SeqCst, SeqCst);
        }

        Ok(())
    }

    fn delete<'a>(&'a self, key: &usize, guard: &'a Guard) -> Result<&'a V, ()> {
        Self::assert_valid_key(*key);
        let (bucket_size, found, mut cursor) = self.find(&key, guard);
        if !found {
            return Err(());
        } else {
            let result = cursor.delete(&guard);
            if result.is_ok() {
                return Ok(unsafe { result.unwrap().assume_init_ref() });
            } else {
                return Err(());
            }
        }
    }
}
