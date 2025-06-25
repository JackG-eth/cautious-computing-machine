// Purpose: Custom double-ended queue (deque) implementation with low-level raw buffer management.

use std::{
    alloc::{self, alloc, Layout},
    collections::VecDeque,
    fmt::Debug,
    marker::PhantomData,
    mem::{ManuallyDrop, MaybeUninit},
    ptr::{self, NonNull},
};

// =====================
// Struct Definitions
// =====================

/// A double-ended queue (deque) with manual memory management.
struct MyDeque<T> {
    buf: RawVec<T>,
    head: usize,
    tail: usize,
    len: usize,
}

/// Immutable reference iterator for MyDeque<T>.
/// Yields &T in logical order, handles wrap-around.
pub struct MyDequeIter<'a, T> {
    head: *const T,
    tail: *const T,
    len: usize,
    buf_cap: usize,
    _marker: PhantomData<&'a T>,
}

/// Mutable reference iterator for MyDeque<T>.
/// Yields &mut T in logical order, handles wrap-around.
pub struct MutMyDequeIter<'a, T> {
    buf: *mut T,
    idx: usize,
    cap: usize,
    remaining: usize,
    marker: PhantomData<&'a T>,
}

/// Consuming iterator for MyDeque<T>.
/// Yields T by value, consuming the deque.
pub struct MyDequeIntoIter<T> {
    ptr: *const T,
    idx: usize,
    cap: usize,
    len: usize,
    _buf: RawVec<T>,
}

/// Raw buffer for manual memory management.
struct RawVec<T> {
    ptr: NonNull<MaybeUninit<T>>,
    cap: usize,
}

// =====================
// Inherent impl blocks
// =====================

// MyDeque<T> inherent methods
impl<T> MyDeque<T> {
    pub fn new() -> Self {
        Self {
            buf: RawVec::new(),
            head: 0,
            tail: 0,
            len: 0,
        }
    }

    pub fn with_capacity(cap: usize) -> Self {
        Self {
            buf: RawVec::with_capacity(cap),
            head: 0,
            tail: 0,
            len: 0,
        }
    }

    pub fn push_back(&mut self, value: T) {
        if self.len == self.buf.cap {
            let (new_head, new_tail) = self.buf.grow(self.head, self.len);
            self.head = new_head;
            self.tail = new_tail;
        }

        self.buf.write(self.tail, value);

        self.tail = (self.tail + 1) % self.buf.cap;
        self.len += 1;
    }

    pub fn push_front(&mut self, value: T) {
        if self.len == self.buf.cap {
            let (new_head, new_tail) = self.buf.grow(self.head, self.len);
            self.head = new_head;
            self.tail = new_tail;
        }

        self.head = (self.head + self.buf.cap - 1) % self.buf.cap; // move head back
        self.buf.write(self.head, value);
        self.len += 1;
    }

    pub fn pop_back(&mut self) -> Option<T> {
        if self.len == 0 {
            None
        } else {
            self.tail = (self.tail - 1 + self.buf.cap) % self.buf.cap;
            self.len -= 1;
            Some(self.buf.read(self.tail))
        }
    }

    pub fn pop_front(&mut self) -> Option<T> {
        if self.len == 0 {
            None
        } else {
            let val = Some(self.buf.read(self.head));
            self.head = (self.head + 1) % self.buf.cap;
            self.len -= 1;
            val
        }
    }

    pub fn peek_back(&self) -> Option<&T> {
        if self.len == 0 {
            None
        } else {
            let tail_idx = (self.tail + self.buf.cap - 1) % self.buf.cap;
            Some(self.buf.read_ref(tail_idx))
        }
    }

    pub fn peek_front(&self) -> Option<&T> {
        if self.len == 0 {
            None
        } else {
            Some(self.buf.read_ref(self.head))
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn capacity(&self) -> usize {
        self.buf.cap
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn is_full(&self) -> bool {
        self.len == self.buf.cap
    }

    // Check if it should panic OB issue
    pub fn get(&self, index: usize) -> Option<&T> {
        if index >= self.len {
            None
        } else {
            let index_ref = (self.head + index) % self.buf.cap;
            Some(self.buf.read_ref(index_ref))
        }
    }

    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        if index >= self.len {
            None
        } else {
            let index_ref = (self.head + index) % self.buf.cap;
            Some(self.buf.read_mut(index_ref))
        }
    }

    pub fn clear(&mut self) {
        for i in 0..self.len {
            let index = (self.head + i) % self.buf.cap;
            self.buf.drop_index(index);
        }
        self.len = 0;
        self.head = 0;
        self.tail = 0;
    }

    pub fn contains(&self, value: &T) -> bool
    where
        T: PartialEq,
    {
        for i in 0..self.len {
            let index = (self.head + i) % self.buf.cap;
            if *value == *self.buf.read_ref(index) {
                return true;
            }
        }
        false
    }

    /*
       append (Moves all the elements of other into self, leaving other empty.)
       retain (Retains only the elements specified by the predicate.)
       retain_mut (Retains only the elements specified by the predicate.)

    */

    /*
        len()	Return number of elements
    capacity()	Total usable capacity
    is_full()	len == cap
    get(index)	Index into deque logically: index 0 is front, etc.
    as_slices()	Return two slices due to wraparound (optional but idiomatic)
         */
}

// RawVec<T> inherent methods
impl<T> RawVec<T> {
    fn new() -> Self {
        let cap = 2;
        // gives us a block of memory of size cap.
        let layout = Layout::array::<MaybeUninit<T>>(cap).unwrap();

        let ptr = unsafe {
            let raw_ptr = alloc(layout) as *mut MaybeUninit<T>;
            NonNull::new(raw_ptr).unwrap_or_else(|| alloc::handle_alloc_error(layout))
        };

        Self { ptr, cap }
    }

    pub fn with_capacity(cap: usize) -> Self {
        let layout = Layout::array::<MaybeUninit<T>>(cap).unwrap();

        let ptr = unsafe {
            let raw_ptr = alloc(layout) as *mut MaybeUninit<T>;
            NonNull::new(raw_ptr).unwrap_or_else(|| alloc::handle_alloc_error(layout))
        };

        Self { ptr, cap }
    }

    /// Grows the buffer for a circular deque.
    ///
    /// This is necessary because the logical order of elements in
    /// a circular buffer can wrap around the physical end of the buffer.
    /// A simple realloc would not reorder the elements correctly.
    ///
    /// So, we:
    /// 1. Allocate a new buffer with double capacity.
    /// 2. Copy existing elements in logical order from old buffer to new buffer.
    /// 3. Update the pointer to the new buffer.
    /// 4. Reset head to 0 and tail to len.
    ///
    /// # Parameters
    /// - head: current index of the front element in the circular buffer.
    /// - len: number of valid elements in the buffer.
    ///
    /// # Returns
    /// Tuple (new_head, new_tail) — these will be (0, len) after growth.
    ///
    /// # Example
    ///
    /// Suppose we have capacity = 4 and buffer holds:
    /// [50, 20, 30, 40]
    ///  ^           ^
    /// tail=3      head=1 (logical order from head to tail)
    ///
    /// The logical order of elements is: 20 (index 1), 30 (2), 40 (3), 50 (0)
    ///
    /// After grow, capacity = 8, and we copy in logical order starting at index 0:
    /// [20, 30, 40, 50, _, _, _, _]
    ///  ^head           ^tail
    /// 0                4 (tail points one past last element)
    fn grow(&mut self, head: usize, len: usize) -> (usize, usize) {
        let new_cap = self.cap * 2;
        let new_layout = Layout::array::<MaybeUninit<T>>(new_cap).unwrap();

        // Allocate new buffer
        let new_ptr = unsafe {
            let raw_ptr = alloc::alloc(new_layout) as *mut MaybeUninit<T>;
            NonNull::new(raw_ptr).unwrap_or_else(|| alloc::handle_alloc_error(new_layout))
        };

        unsafe {
            // Copy elements from old buffer to new buffer in logical order
            for i in 0..len {
                // Calculate source index, wrapping around old capacity
                let src_idx = (head + i) % self.cap;
                // Pointers for source and destination
                let src = self.ptr.as_ptr().add(src_idx);
                let dst = new_ptr.as_ptr().add(i);
                // Copy the element without dropping
                dst.write(src.read());
            }

            // Deallocate old buffer
            let old_layout = Layout::array::<MaybeUninit<T>>(self.cap).unwrap();
            alloc::dealloc(self.ptr.as_ptr() as *mut u8, old_layout);
        }

        // Update pointer and capacity
        self.ptr = new_ptr;
        self.cap = new_cap;

        // Reset head to 0 and tail to len, reflecting new linear layout
        (0, len)
    }

    fn write(&mut self, index: usize, value: T) {
        unsafe {
            (*self.ptr.as_ptr().add(index)).write(value);
        }
    }

    fn read(&self, index: usize) -> T {
        unsafe { ptr::read((*self.ptr.as_ptr().add(index)).as_ptr()) }
    }

    fn read_mut(&mut self, index: usize) -> &mut T {
        unsafe { &mut *self.ptr.as_ptr().add(index).cast::<T>() }
    }

    fn read_ref(&self, index: usize) -> &T {
        unsafe { &*self.ptr.as_ptr().add(index).cast::<T>() }
    }

    fn drop_index(&mut self, index: usize) {
        unsafe {
            ptr::drop_in_place(self.ptr.as_ptr().add(index).cast::<T>());
        }
    }
}

// =====================
// Trait Implementations
// =====================

// Iterator for MyDequeIter<'a, T>
impl<'a, T> Iterator for MyDequeIter<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.len == 0 {
            None
        } else {
            let item = unsafe { &*self.head };
            // Advance head, wrapping if needed
            self.head = if (self.head as usize + 1) % self.buf_cap == self.tail as usize {
                self.tail // End of iteration
            } else {
                unsafe { self.head.add(1) }
            };
            self.len -= 1;
            Some(item)
        }
    }
}

// IntoIterator for &MyDeque<T>
impl<'a, T> IntoIterator for &'a MyDeque<T> {
    type Item = &'a T;

    type IntoIter = MyDequeIter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        let head = unsafe { self.buf.ptr.add(self.head).as_ptr() } as *const T;
        let tail = unsafe { self.buf.ptr.add(self.tail).as_ptr() } as *const T;

        MyDequeIter {
            head,
            tail,
            len: self.len,
            buf_cap: self.buf.cap,
            _marker: PhantomData,
        }
    }
}

// Iterator for MutMyDequeIter<'a, T>
impl<'a, T> Iterator for MutMyDequeIter<'a, T> {
    type Item = &'a mut T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            None
        } else {
            let item = unsafe { self.buf.add(self.idx).as_mut() };
            self.idx = (self.idx + 1) % self.cap;
            self.remaining -= 1;
            item
        }
    }
}

// IntoIterator for &mut MyDeque<T>
impl<'a, T> IntoIterator for &'a mut MyDeque<T> {
    type Item = &'a mut T;

    type IntoIter = MutMyDequeIter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        let buf = unsafe { self.buf.ptr.add(self.head).as_ptr() as *mut T };
        MutMyDequeIter {
            buf,
            idx: self.head,
            cap: self.buf.cap,
            remaining: self.len(),
            marker: PhantomData,
        }
    }
}

// Iterator for MyDequeIntoIter<T>
impl<T> Iterator for MyDequeIntoIter<T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.len == 0 {
            None
        } else {
            let item = unsafe { ptr::read(self.ptr.add(self.idx)) };
            self.idx = (self.idx + 1) % self.cap;
            self.len -= 1;
            Some(item)
        }
    }
}

// IntoIterator for MyDeque<T>
impl<T> IntoIterator for MyDeque<T> {
    type Item = T;

    type IntoIter = MyDequeIntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        // Prevent MyVec's Drop as we're dropping it with MyVecIntoIntoIter
        // stops double drop
        let raw_self = ManuallyDrop::new(self);

        unsafe {
            let buf = ptr::read(&raw_self.buf);

            let ptr = buf.ptr.as_ptr() as *const T;

            MyDequeIntoIter {
                ptr,
                idx: raw_self.head,
                cap: raw_self.buf.cap,
                len: raw_self.len,
                _buf: buf,
            }
        }
    }
}

impl<T: Ord> Ord for MyDeque<T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        let mut self_iter = self.into_iter();
        let mut other_iter = other.into_iter();

        loop {
            match (self_iter.next(), other_iter.next()) {
                (None, None) => return std::cmp::Ordering::Equal,
                (None, Some(_)) => return std::cmp::Ordering::Less,
                (Some(_), None) => return std::cmp::Ordering::Greater,
                (Some(a), Some(b)) => {
                    let cmp = a.cmp(b);
                    if cmp != std::cmp::Ordering::Equal {
                        return cmp;
                    }
                }
            }
        }
    }
}

impl<T: PartialOrd> PartialOrd for MyDeque<T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        let mut self_iter = self.into_iter();
        let mut other_iter = other.into_iter();

        loop {
            match (self_iter.next(), other_iter.next()) {
                (None, None) => return Some(std::cmp::Ordering::Equal),
                (None, Some(_)) => return Some(std::cmp::Ordering::Less),
                (Some(_), None) => return Some(std::cmp::Ordering::Greater),
                (Some(a), Some(b)) => match a.partial_cmp(b) {
                    Some(std::cmp::Ordering::Equal) => continue,
                    Some(cmp) => return Some(cmp),
                    None => return None,
                },
            }
        }
    }
}

// Drop for MyDequeIntoIter<T>
impl<T> Drop for MyDequeIntoIter<T> {
    fn drop(&mut self) {
        while self.len != 0 {
            unsafe {
                ptr::drop_in_place(self.ptr.add(self.idx) as *mut T);
            }
            self.idx = (self.idx + 1) % self.cap;
            self.len -= 1;
        }
    }
}

// Drop for MyDeque<T>
impl<T> Drop for MyDeque<T> {
    fn drop(&mut self) {
        for i in 0..self.len {
            unsafe {
                let index = (self.head + i) % self.buf.cap;
                ptr::drop_in_place((*self.buf.ptr.as_ptr().add(index)).as_mut_ptr());
            }
        }
    }
}

// Drop for RawVec<T>
impl<T> Drop for RawVec<T> {
    fn drop(&mut self) {
        if self.cap == 0 {
            return;
        }

        let layout = Layout::array::<MaybeUninit<T>>(self.cap).unwrap();

        unsafe {
            alloc::dealloc(self.ptr.as_ptr() as *mut u8, layout);
        }
    }
}

// Clone for MyDeque<T>
impl<T: Clone> Clone for MyDeque<T> {
    fn clone(&self) -> Self {
        let mut new = MyDeque::new();
        for i in 0..self.len {
            let index = (self.head + i) % self.buf.cap;
            let val = self.buf.read_ref(index);
            new.push_back(val.clone());
        }
        new
    }
}

// PartialEq for MyDeque<T>
impl<T: PartialEq> PartialEq for MyDeque<T> {
    fn eq(&self, other: &Self) -> bool {
        if self.len != other.len {
            return false;
        }
        for i in 0..self.len {
            if self.get(i) != other.get(i) {
                return false;
            }
        }
        true
    }
}

// Eq for MyDeque<T>
impl<T: Eq> Eq for MyDeque<T> {}

// Extend for MyDeque<T>
impl<T> Extend<T> for MyDeque<T> {
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        for val in iter {
            self.push_back(val);
        }
    }
}

// FromIterator for MyDeque<T>
impl<T: Clone> FromIterator<T> for MyDeque<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let mut new_deque = MyDeque::new();
        for value in iter {
            new_deque.push_back(value.clone());
        }
        new_deque
    }
}

// From<Vec<T>> for MyDeque<T>
impl<T> From<Vec<T>> for MyDeque<T> {
    fn from(vec: Vec<T>) -> Self {
        let mut new_deque = MyDeque::with_capacity(vec.len());
        for value in vec {
            new_deque.push_back(value);
        }
        new_deque
    }
}

// From<&[T]> for MyDeque<T>
impl<T: Clone> From<&[T]> for MyDeque<T> {
    fn from(vec: &[T]) -> Self {
        let mut new_deque = MyDeque::with_capacity(vec.len());
        for value in vec {
            new_deque.push_back(value.clone());
        }
        new_deque
    }
}

// Debug for MyDeque<T>
impl<T: Debug> Debug for MyDeque<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MyDeque")
            .field("buf", &self.buf)
            .field("head", &self.head)
            .field("tail", &self.tail)
            .field("len", &self.len)
            .finish()
    }
}

// Debug for RawVec<T>
impl<T: Debug> Debug for RawVec<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RawVec")
            .field("ptr", &self.ptr)
            .field("cap", &self.cap)
            .finish()
    }
}

// =====================
// Tests
// =====================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_push_back_basic() {
        let mut deque = MyDeque::new();

        deque.push_back(10);
        deque.push_back(20);
        deque.push_back(30);

        // After three pushes at back:
        // Elements: [10, 20, 30]
        // head = 0, tail = 3, len = 3
        assert_eq!(deque.len, 3);
        assert_eq!(deque.head, 0);
        assert_eq!(deque.tail, 3 % deque.buf.cap);

        // Buffer contains 10 at index 0, 20 at 1, 30 at 2
        unsafe {
            assert_eq!((*deque.buf.ptr.as_ptr().add(0)).assume_init(), 10);
            assert_eq!((*deque.buf.ptr.as_ptr().add(1)).assume_init(), 20);
            assert_eq!((*deque.buf.ptr.as_ptr().add(2)).assume_init(), 30);
        }
    }

    #[test]
    fn test_push_front_basic() {
        let mut deque = MyDeque::new();

        deque.push_front(10);
        deque.push_front(20);
        deque.push_front(30);

        // After three pushes at front:
        // Elements inserted at indices moving backward (head wraps around)
        // Expect head to move backward with wrap
        assert_eq!(deque.len, 3);

        // Since head moves backward on push_front,
        // it should be (0 + cap - 3) % cap = (0 + 2 - 3) % 2 = 1 (with initial cap=2)
        // but capacity grows when full, so capacity should be larger.
        // Just check that all elements are present.

        // Elements logically: [30, 20, 10]
        unsafe {
            // Check values at correct positions
            let cap = deque.buf.cap;
            let head = deque.head;
            for i in 0..deque.len {
                let index = (head + i) % cap;
                let val = (*deque.buf.ptr.as_ptr().add(index)).assume_init();
                // Since we pushed 30, 20, 10 at front in that order,
                // The oldest inserted value (10) should be at tail-1
                assert!(val == 10 || val == 20 || val == 30);
            }
        }
    }

    #[test]
    fn test_push_back_and_front_mix() {
        let mut deque = MyDeque::new();

        deque.push_back(10); // index 0
        deque.push_back(20); // index 1
        deque.push_front(30); // head moves backward, so index cap-1
        deque.push_front(40); // head moves backward again

        // After these operations:
        // Elements order logically: [40, 30, 10, 20]
        assert_eq!(deque.len, 4);

        // Check values at expected positions by iterating from head forward
        unsafe {
            let cap = deque.buf.cap;
            let mut idx = deque.head;
            let mut values = Vec::new();
            for _ in 0..deque.len {
                values.push((*deque.buf.ptr.as_ptr().add(idx)).assume_init());
                idx = (idx + 1) % cap;
            }
            assert_eq!(values, vec![40, 30, 10, 20]);
        }
    }

    #[test]
    fn test_grow_capacity() {
        let mut deque = MyDeque::new();

        // Push enough elements to trigger growth (initial cap=2)
        deque.push_back(1);
        deque.push_back(2);
        deque.push_back(3); // triggers grow
        deque.push_back(4);

        assert!(deque.buf.cap >= 4);
        assert_eq!(deque.len, 4);

        // Check elements order is maintained
        unsafe {
            let mut idx = deque.head;
            let mut values = Vec::new();
            for _ in 0..deque.len {
                values.push((*deque.buf.ptr.as_ptr().add(idx)).assume_init());
                idx = (idx + 1) % deque.buf.cap;
            }
            assert_eq!(values, vec![1, 2, 3, 4]);
        }
    }

    #[test]
    fn test_push_back_and_peek() {
        let mut deque = MyDeque::new();
        deque.push_back(10);
        deque.push_back(20);

        assert_eq!(deque.peek_front(), Some(&10));
        assert_eq!(deque.peek_back(), Some(&20));
    }

    #[test]
    fn test_push_front_and_peek() {
        let mut deque = MyDeque::new();
        deque.push_front(10);
        deque.push_front(20);

        // Because push_front inserts at the "front", first peek_front is 20
        assert_eq!(deque.peek_front(), Some(&20));
        assert_eq!(deque.peek_back(), Some(&10));
    }

    #[test]
    fn test_pop_front_and_pop_back() {
        let mut deque = MyDeque::new();
        deque.push_back(1);
        deque.push_back(2);
        deque.push_back(3);

        assert_eq!(deque.pop_front(), Some(1));
        assert_eq!(deque.pop_back(), Some(3));
        assert_eq!(deque.pop_front(), Some(2));
        assert_eq!(deque.pop_back(), None);
    }

    #[test]
    fn test_peek_on_empty_deque() {
        let deque: MyDeque<i32> = MyDeque::new();
        assert_eq!(deque.peek_front(), None);
        assert_eq!(deque.peek_back(), None);
    }

    #[test]
    fn test_push_pop_mix() {
        let mut deque = MyDeque::new();

        deque.push_back(1);
        deque.push_front(2);
        deque.push_back(3);
        deque.push_front(4);

        // Logical order should be [4, 2, 1, 3]
        assert_eq!(deque.peek_front(), Some(&4));
        assert_eq!(deque.peek_back(), Some(&3));

        assert_eq!(deque.pop_front(), Some(4));
        assert_eq!(deque.pop_back(), Some(3));
        assert_eq!(deque.pop_front(), Some(2));
        assert_eq!(deque.pop_back(), Some(1));
        assert_eq!(deque.pop_back(), None);
    }

    #[test]
    fn test_grow_and_peek() {
        let mut deque = MyDeque::new();
        // Push enough elements to trigger growth
        for i in 0..10 {
            deque.push_back(i);
        }

        assert_eq!(deque.len, 10);
        assert_eq!(deque.peek_front(), Some(&0));
        assert_eq!(deque.peek_back(), Some(&9));

        // Pop a few and check peeks update correctly
        let front = deque.pop_front();
        let back = deque.pop_back();

        dbg!(front);
        dbg!(back);

        assert_eq!(deque.peek_front(), Some(&1));
        assert_eq!(deque.peek_back(), Some(&8));
    }

    #[test]
    fn test_clear() {
        let mut deque = MyDeque::new();

        deque.push_back(10);
        deque.push_back(20);
        deque.push_back(30);

        deque.clear();

        assert!(deque.is_empty())
    }

    #[test]
    fn test_contains() {
        let mut deque = MyDeque::new();

        deque.push_back(10);
        deque.push_back(20);
        deque.push_back(30);

        assert!(deque.contains(&10));
        assert!(!deque.contains(&40));
    }

    #[test]
    fn test_extend() {
        let mut deque = MyDeque::new();
        deque.push_back(1);
        deque.push_back(2);
        deque.extend(vec![3, 4, 5]);
        let collected: Vec<_> = (0..deque.len()).map(|i| *deque.get(i).unwrap()).collect();
        assert_eq!(collected, vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn test_clone() {
        let mut deque = MyDeque::new();
        deque.push_back(10);
        deque.push_back(20);
        let cloned = deque.clone();
        assert_eq!(deque, cloned);
        // Mutate original, cloned should not change
        deque.push_back(30);
        assert_ne!(deque, cloned);
    }

    #[test]
    fn test_drop() {
        use std::sync::{Arc, Mutex};
        struct DropCounter(Arc<Mutex<usize>>);
        impl Drop for DropCounter {
            fn drop(&mut self) {
                let mut count = self.0.lock().unwrap();
                *count += 1;
            }
        }
        let counter = Arc::new(Mutex::new(0));
        {
            let mut deque = MyDeque::new();
            for _ in 0..5 {
                deque.push_back(DropCounter(counter.clone()));
            }
            // When deque goes out of scope, all DropCounter should be dropped
        }
        assert_eq!(*counter.lock().unwrap(), 5);
    }

    #[test]
    fn test_ref_iter() {
        let mut deque = MyDeque::new();
        let mut iter_vec = vec![];

        deque.push_back(10);
        deque.push_back(20);
        deque.push_back(30);

        for val in &deque {
            iter_vec.push(val);
        }

        assert_eq!(vec![&10, &20, &30], iter_vec);
    }

    #[test]
    fn test_mydeque_iter() {
        let mut deque = MyDeque::new();
        deque.push_back(1);
        deque.push_back(2);
        deque.push_back(3);
        let collected: Vec<_> = (&deque).into_iter().cloned().collect();
        assert_eq!(collected, vec![1, 2, 3]);
    }

    #[test]
    fn test_mydeque_iter_mut() {
        let mut deque = MyDeque::new();
        deque.push_back(10);
        deque.push_back(20);
        deque.push_back(30);
        for val in &mut deque {
            *val += 1;
        }
        let collected: Vec<_> = (&deque).into_iter().cloned().collect();
        assert_eq!(collected, vec![11, 21, 31]);
    }

    #[test]
    fn test_mydeque_into_iter() {
        let mut deque = MyDeque::new();
        deque.push_back(100);
        deque.push_back(200);
        deque.push_back(300);
        let collected: Vec<_> = deque.into_iter().collect();
        assert_eq!(collected, vec![100, 200, 300]);
    }

    #[test]
    fn test_mydeque_into_iter_drop() {
        use std::sync::{Arc, Mutex};
        struct DropCounter(Arc<Mutex<usize>>);
        impl Drop for DropCounter {
            fn drop(&mut self) {
                let mut count = self.0.lock().unwrap();
                *count += 1;
            }
        }
        let counter = Arc::new(Mutex::new(0));
        {
            let mut deque = MyDeque::new();
            for _ in 0..4 {
                deque.push_back(DropCounter(counter.clone()));
            }
            // Only consume part of the iterator
            let mut iter = deque.into_iter();
            let _ = iter.next(); // consume one
                                 // When iter is dropped, remaining elements should be dropped
        }
        assert_eq!(*counter.lock().unwrap(), 4);
    }
}