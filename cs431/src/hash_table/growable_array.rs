//! Growable array.

use core::fmt::Debug;
use core::mem::{self, ManuallyDrop};
use core::sync::atomic::Ordering::*;
use crossbeam_epoch::{Atomic, Guard, Owned, Shared};
use std::sync::atomic::Ordering;
use std::usize;

/// Growable array of `Atomic<T>`.
///
/// This is more complete version of the dynamic sized array from the paper. In the paper, the
/// segment table is an array of arrays (segments) of pointers to the elements. In this
/// implementation, a segment contains the pointers to the elements **or other child segments**. In
/// other words, it is a tree that has segments as internal nodes.
///
/// # Example run
///
/// Suppose `SEGMENT_LOGSIZE = 3` (segment size 8).
///
/// When a new `GrowableArray` is created, `root` is initialized with `Atomic::null()`.
///
/// ```text
///                          +----+
///                          |root|
///                          +----+
/// ```
///
/// When you store element `cat` at the index `0b001`, it first initializes a segment.
///
/// ```text
///                          +----+
///                          |root|
///                          +----+
///                            | height: 1
///                            v
///                 +---+---+---+---+---+---+---+---+
///                 |111|110|101|100|011|010|001|000|
///                 +---+---+---+---+---+---+---+---+
///                                           |
///                                           v
///                                         +---+
///                                         |cat|
///                                         +---+
/// ```
///
/// When you store `fox` at `0b111011`, it is clear that there is no room for indices larger than
/// `0b111`. So it first allocates another segment for upper 3 bits and moves the previous root
/// segment (`0b000XXX` segment) under the `0b000XXX` branch of the the newly allocated segment.
///
/// ```text
///                          +----+
///                          |root|
///                          +----+
///                            | height: 2
///                            v
///                 +---+---+---+---+---+---+---+---+
///                 |111|110|101|100|011|010|001|000|
///                 +---+---+---+---+---+---+---+---+
///                                               |
///                                               v
///                                      +---+---+---+---+---+---+---+---+
///                                      |111|110|101|100|011|010|001|000|
///                                      +---+---+---+---+---+---+---+---+
///                                                                |
///                                                                v
///                                                              +---+
///                                                              |cat|
///                                                              +---+
/// ```
///
/// And then, it allocates another segment for `0b111XXX` indices.
///
/// ```text
///                          +----+
///                          |root|
///                          +----+
///                            | height: 2
///                            v
///                 +---+---+---+---+---+---+---+---+
///                 |111|110|101|100|011|010|001|000|
///                 +---+---+---+---+---+---+---+---+
///                   |                           |
///                   v                           v
/// +---+---+---+---+---+---+---+---+    +---+---+---+---+---+---+---+---+
/// |111|110|101|100|011|010|001|000|    |111|110|101|100|011|010|001|000|
/// +---+---+---+---+---+---+---+---+    +---+---+---+---+---+---+---+---+
///                   |                                            |
///                   v                                            v
///                 +---+                                        +---+
///                 |fox|                                        |cat|
///                 +---+                                        +---+
/// ```
///
/// Finally, when you store `owl` at `0b000110`, it traverses through the `0b000XXX` branch of the
/// height 2 segment and arrives at its `0b110` leaf.
///
/// ```text
///                          +----+
///                          |root|
///                          +----+
///                            | height: 2
///                            v
///                 +---+---+---+---+---+---+---+---+
///                 |111|110|101|100|011|010|001|000|
///                 +---+---+---+---+---+---+---+---+
///                   |                           |
///                   v                           v
/// +---+---+---+---+---+---+---+---+    +---+---+---+---+---+---+---+---+
/// |111|110|101|100|011|010|001|000|    |111|110|101|100|011|010|001|000|
/// +---+---+---+---+---+---+---+---+    +---+---+---+---+---+---+---+---+
///                   |                        |                   |
///                   v                        v                   v
///                 +---+                    +---+               +---+
///                 |fox|                    |owl|               |cat|
///                 +---+                    +---+               +---+
/// ```
///
/// When the array is dropped, only the segments are dropped and the **elements must not be
/// dropped/deallocated**.
///
/// ```text
///                 +---+                    +---+               +---+
///                 |fox|                    |owl|               |cat|
///                 +---+                    +---+               +---+
/// ```
///
/// Instead, it should be handled by the container that the elements actually belong to. For
/// example, in `SplitOrderedList` the destruction of elements are handled by the inner `List`.
#[derive(Debug)]
pub struct GrowableArray<T> {
    root: Atomic<Segment<T>>,
}

const SEGMENT_LOGSIZE: usize = 10;

/// A fixed size array of atomic pointers to other `Segment<T>` or `T`.
///
/// Each segment is either a child segment with pointers to `Segment<T>` or an element segment with
/// pointers to `T`. This is determined by the height of this segment in the main array, which one
/// needs to track separately. For example, use the main array root's tag.
///
/// Since destructing `Segment<T>` requires its height information, it is not recommended to
/// implement `Drop` for this union. Rather, have a custom deallocate method that accounts for the
/// height of the segment.
union Segment<T> {
    children: ManuallyDrop<[Atomic<Segment<T>>; 1 << SEGMENT_LOGSIZE]>,
    elements: ManuallyDrop<[Atomic<T>; 1 << SEGMENT_LOGSIZE]>,
}

impl<T> Segment<T> {
    /// Create a new segment filled with null pointers. It is up to the callee to whether to use
    /// this as a children or an element segment.
    fn new() -> Owned<Self> {
        Owned::new(
            // SAFETY: An array of null pointers can be interperted as either an element segment or
            // an children segment.
            unsafe { mem::zeroed() },
        )
    }

    // todo: change signature to somethink like `drop(self)` or `&self` ?
    fn drop_segments(segment: &mut Shared<Segment<T>>, height: usize, guard: &Guard) {
        if !segment.is_null() {
            let mut owned = unsafe { segment.into_owned() };

            if height > 1 {
                let children = unsafe { &*owned.children };
                for child in children.iter() {
                    Self::drop_segments(&mut child.load(SeqCst, guard), height - 1, guard);
                }
            } else {
                unsafe {
                    ManuallyDrop::drop(&mut (owned.children));
                }
            }

            drop(owned);
        }
    }
}

impl<T> Debug for Segment<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Segment")
    }
}

impl<T> Drop for GrowableArray<T> {
    /// Deallocate segments, but not the individual elements.
    fn drop(&mut self) {
        let guard = crossbeam_epoch::pin();
        let mut root = self.root.load(SeqCst, &guard);
        let init_height = root.tag();
        Segment::<T>::drop_segments(&mut root, init_height, &guard);
    }
}

impl<T> Default for GrowableArray<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> GrowableArray<T> {
    /// Create a new growable array.
    pub fn new() -> Self {
        Self {
            root: Atomic::null(),
        }
    }

    /// Returns the reference to the `Atomic` pointer at `index`. Allocates new segments if
    /// necessary.
    pub fn get<'g>(&self, mut index: usize, guard: &'g Guard) -> &'g Atomic<T> {
        let mut root = self.root.load(Ordering::SeqCst, guard);
        if root.is_null() {
            let new_root = Segment::new().with_tag(1);
            // todo: check drop
            let result = self
                .root
                .compare_exchange(
                    Shared::null(),
                    new_root,
                    Ordering::SeqCst,
                    Ordering::SeqCst,
                    guard,
                )
                .map_err(|e| drop(e.new));
            root = self.root.load(Ordering::SeqCst, guard);
        }

        let mut required_height: usize = 1;
        {
            let mut max_index_at_depth = (1 << SEGMENT_LOGSIZE) - 1;

            while index > max_index_at_depth {
                required_height += 1;
                max_index_at_depth = (1 << (SEGMENT_LOGSIZE * required_height)) - 1;
            }
        }

        let mut current_max_height = root.tag();

        // dbg!(index);
        // dbg!(current_max_height);
        // dbg!(required_height);

        if required_height > current_max_height {
            let new_root = Segment::new().with_tag(required_height).into_shared(&guard);
            let mut new_segment = unsafe { new_root.deref() };

            while required_height > current_max_height {
                if required_height - 1 == current_max_height {
                    unsafe {
                        new_segment.children[0].store(root, SeqCst);
                        // todo: handle failure
                        let _ = self
                            .root
                            .compare_exchange(root, new_root, SeqCst, SeqCst, &guard);
                    }
                } else {
                    unsafe {
                        new_segment.children[0].store(Segment::new(), SeqCst);
                        new_segment = new_segment.children[0].load(SeqCst, &guard).deref();
                    }
                }
                required_height = required_height - 1;
            }
        }

        // regular search
        root = self.root.load(SeqCst, &guard);
        current_max_height = root.tag();
        let mut segment = unsafe { root.deref() };

        while current_max_height > 1 {
            let segment_index = index >> (SEGMENT_LOGSIZE * (current_max_height - 1));
            index &= (1 << (SEGMENT_LOGSIZE * (current_max_height - 1))) - 1;

            // dbg!(segment_index);
            // dbg!(index);

            let next_segment =
                unsafe { segment.children[segment_index].load(Ordering::SeqCst, guard) };
            if next_segment.is_null() {
                let new_segment = Segment::new();
                unsafe {
                    // todo: handle failure
                    let _ = segment.children[segment_index].compare_exchange(
                        Shared::null(),
                        new_segment,
                        Ordering::SeqCst,
                        Ordering::SeqCst,
                        &guard,
                    );
                    segment = segment.children[segment_index].load(SeqCst, &guard).deref();
                }
            } else {
                segment = unsafe { next_segment.deref() };
            }

            current_max_height = current_max_height - 1;
        }

        unsafe { &segment.elements[index] }
    }
}
