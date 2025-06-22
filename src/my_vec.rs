use std::{
    alloc::{self, Layout},
    collections::btree_map::RangeMut,
    marker::PhantomData,
    mem::{ManuallyDrop, MaybeUninit},
    ops::{Add, Index, IndexMut, Range, RangeInclusive},
    path::Iter,
    ptr::{self, NonNull},
    slice::{from_raw_parts, from_raw_parts_mut},
};

/*
RawVec<T>:

- Low-level memory allocator & manager.
- Handles allocation, reallocation, and deallocation.
- Deals with capacity, pointer math, and unsafe internals.

MyVec<T>:

- High-level, safe(ish) user-facing API.
- Tracks what portion of memory is actually in use.
- Provides safe methods like push, pop, get, etc.
*/

#[derive(Debug)]
struct MyVec<T> {
    data: RawVec<T>,
    len: usize,
}

impl<T> MyVec<T> {
    fn new() -> Self {
        MyVec {
            data: RawVec::new(),
            len: 0,
        }
    }

    fn push(&mut self, value: T) {
        if self.len == self.data.cap {
            self.data.grow();
        }

        self.data.write(self.len, value);
        self.len += 1;
    }

    fn pop(&mut self) -> Option<T> {
        if self.len == 0 {
            None
        } else {
            self.len -= 1;
            Some(self.data.read_last(self.len))
        }
    }

    fn get(&self, index: usize) -> Option<&T> {
        if index >= self.len {
            None
        } else {
            unsafe {
                let ptr = self.data.ptr.as_ptr().add(index);
                Some(&*ptr.cast::<T>())
            }
        }
    }

    // use cast to conver the MaybeUninit into T
    fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        if index >= self.len {
            None
        } else {
            Some(self.data.read_mut(index))
        }
    }

    fn get_len(&self) -> usize {
        self.len
    }

    fn get_capacity(&self) -> usize {
        self.data.cap
    }

    fn as_slice(&self) -> &[T] {
        self.data.slice(self.len)
    }

    fn as_mut_slice(&mut self) -> &mut [T] {
        self.data.slice_mut(self.len)
    }

    fn insert(&mut self, index: usize, value: T) {
        assert!(index <= self.len);
        if self.len == self.data.cap {
            self.data.grow();
        }

        self.data.write_pos(index, value, self.len);

        self.len += 1;
    }

    fn remove(&mut self, index: usize) -> T {
        assert!(index < self.len);
        let val = self.data.read(index);
        self.data.remove_pos(index, self.len);
        self.len -= 1;
        val
    }
}

impl<T> Drop for MyVec<T> {
    fn drop(&mut self) {
        // Drop each initialized element (in reverse for safety)
        for i in (0..self.len).rev() {
            unsafe {
                ptr::drop_in_place((*self.data.ptr.as_ptr().add(i)).as_mut_ptr());
            }
        }
        // RawVec will handle memory deallocation
    }
}

impl<T> Clone for MyVec<T>
where
    T: Clone,
{
    fn clone(&self) -> Self {
        let mut new = MyVec::new();
        for i in 0..self.len {
            let val = self.data.read(i); // move out temporarily
            new.push(val.clone());
            std::mem::forget(val); // prevent double-drop
        }
        new
    }
}

impl<T> Index<usize> for MyVec<T> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        self.get(index).expect("index out of bounds")
    }
}

impl<T> IndexMut<usize> for MyVec<T> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        self.get_mut(index).expect("index out of bounds")
    }
}

impl<T> Index<Range<usize>> for MyVec<T> {
    type Output = [T];

    fn index(&self, range: Range<usize>) -> &Self::Output {
        &self.as_slice()[range]
    }
}

impl<T> IndexMut<Range<usize>> for MyVec<T> {
    fn index_mut(&mut self, range: Range<usize>) -> &mut Self::Output {
        &mut self.as_mut_slice()[range]
    }
}

impl<T> Index<RangeInclusive<usize>> for MyVec<T> {
    type Output = [T];

    fn index(&self, range: RangeInclusive<usize>) -> &Self::Output {
        &self.as_slice()[range]
    }
}

pub struct MyVecIter<'a, T> {
    start: *const T,
    end: *const T,
    _marker: PhantomData<&'a T>,
}

impl<'a, T> Iterator for MyVecIter<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.start == self.end {
            None
        } else {
            let item = unsafe { &*self.start };
            self.start = unsafe { self.start.add(1) };
            Some(item)
        }
    }
}

impl<'a, T> IntoIterator for &'a MyVec<T> {
    type Item = &'a T;
    type IntoIter = MyVecIter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        let start = self.data.ptr.as_ptr() as *const T;
        let end = unsafe { start.add(self.len) };

        MyVecIter {
            start,
            end,
            _marker: std::marker::PhantomData,
        }
    }
}

pub struct MutMyVecIter<'a, T> {
    start: *mut T,
    end: *mut T,
    _marker: PhantomData<&'a T>,
}

impl<'a, T> Iterator for MutMyVecIter<'a, T> {
    type Item = &'a mut T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.start == self.end {
            None
        } else {
            let item = unsafe { self.start.as_mut() };
            self.start = unsafe { self.start.add(1) };
            item
        }
    }
}

impl<'a, T> IntoIterator for &'a mut MyVec<T> {
    type Item = &'a mut T;

    type IntoIter = MutMyVecIter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        let start = self.data.ptr.as_ptr() as *mut T;
        let end = unsafe { start.add(self.len) };
        MutMyVecIter {
            start,
            end,
            _marker: PhantomData,
        }
    }
}

pub struct MyVecIntoIntoIter<T> {
    ptr: *const T,
    index: usize,
    len: usize,
    _buf: RawVec<T>,
}

impl<T> Iterator for MyVecIntoIntoIter<T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index == self.len {
            None
        } else {
            let item = self._buf.read(self.index);
            self.index = self.index.add(1);

            Some(item)
        }
    }
}

impl<T> IntoIterator for MyVec<T> {
    type Item = T;
    type IntoIter = MyVecIntoIntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        // Prevent MyVec's Drop as we're dropping it with MyVecIntoIntoIter
        let raw_self = ManuallyDrop::new(self);

        unsafe {
            // Move out fields manually
            let data = ptr::read(&raw_self.data);
            let len = ptr::read(&raw_self.len);

            let ptr = data.ptr.as_ptr() as *const T;

            MyVecIntoIntoIter {
                ptr,
                index: 0,
                len,
                _buf: data,
            }
        }
    }
}

impl<T> Drop for MyVecIntoIntoIter<T> {
    fn drop(&mut self) {
        for i in self.index..self.len {
            unsafe {
                ptr::drop_in_place(self.ptr.add(i) as *mut T);
            }
        }
    }
}

#[derive(Debug)]
struct RawVec<T> {
    ptr: NonNull<MaybeUninit<T>>,
    cap: usize,
}

impl<T> RawVec<T> {
    fn new() -> Self {
        let cap = 2;
        let layout = Layout::array::<MaybeUninit<T>>(cap).unwrap();
        let ptr = unsafe {
            let raw_ptr = alloc::alloc(layout) as *mut MaybeUninit<T>;
            NonNull::new(raw_ptr).unwrap_or_else(|| alloc::handle_alloc_error(layout))
        };

        Self { ptr, cap }
    }

    fn grow(&mut self) {
        let new_cap = self.cap * 2;
        let new_layout = Layout::array::<MaybeUninit<T>>(new_cap).unwrap();
        let old_layout = Layout::array::<MaybeUninit<T>>(self.cap).unwrap();

        unsafe {
            let new_ptr =
                alloc::realloc(self.ptr.as_ptr() as *mut u8, old_layout, new_layout.size())
                    as *mut MaybeUninit<T>;

            self.ptr =
                NonNull::new(new_ptr).unwrap_or_else(|| alloc::handle_alloc_error(new_layout));
        }

        self.cap = new_cap;
    }

    fn write(&mut self, index: usize, value: T) {
        unsafe {
            (*self.ptr.as_ptr().add(index)).write(value);
        }
    }

    fn write_pos(&mut self, index: usize, value: T, len: usize) {
        unsafe {
            // so this means copy all the values up the inde
            ptr::copy(
                self.ptr.as_ptr().add(index),
                self.ptr.as_ptr().add(index + 1),
                len - index,
            );
            self.write(index, value);
        }
    }

    // Shifts the array left
    // remove_pos(2,4)
    // [39,40,209,30]
    // [39,40,30,_]
    fn remove_pos(&mut self, index: usize, len: usize) {
        unsafe {
            ptr::copy(
                self.ptr.as_ptr().add(index + 1),
                self.ptr.as_ptr().add(index),
                len - index - 1,
            );
        }
    }

    fn read_mut(&mut self, index: usize) -> &mut T {
        unsafe { &mut *self.ptr.as_ptr().add(index).cast::<T>() }
    }

    fn read_last(&self, len: usize) -> T {
        unsafe { ptr::read((*self.ptr.as_ptr().add(len)).as_ptr()) }
    }

    fn read(&self, index: usize) -> T {
        unsafe { ptr::read((*self.ptr.as_ptr().add(index)).as_ptr()) }
    }

    fn slice(&self, len: usize) -> &[T] {
        unsafe { from_raw_parts(self.ptr.as_ptr() as *const T, len) }
    }

    fn slice_mut(&mut self, len: usize) -> &mut [T] {
        unsafe { from_raw_parts_mut(self.ptr.as_ptr() as *mut T, len) }
    }
}

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

#[cfg(test)]
mod vec_tests {
    use super::*;

    #[test]
    fn test_push_and_get() {
        let mut vec = MyVec::new();

        vec.push(10);
        vec.push(20);

        assert_eq!(vec.get(0), Some(&10));
        assert_eq!(vec.get(1), Some(&20));
        assert_eq!(vec.get(2), None); // OOB
    }

    #[test]
    fn test_pop() {
        let mut vec = MyVec::new();

        vec.push(1);
        vec.push(2);
        vec.push(3);

        assert_eq!(vec.pop(), Some(3));
        assert_eq!(vec.pop(), Some(2));
        assert_eq!(vec.pop(), Some(1));
        assert_eq!(vec.pop(), None); // now empty
    }

    #[test]
    fn test_capacity_grows() {
        let mut vec = MyVec::new();

        let initial_cap = vec.get_capacity();

        for i in 0..(initial_cap + 1) {
            vec.push(i as i32);
        }

        assert!(vec.get_capacity() > initial_cap);
        assert_eq!(vec.get_len(), initial_cap + 1);
    }

    #[test]
    fn test_clone() {
        let mut vec1 = MyVec::new();
        vec1.push(5);
        vec1.push(10);

        let vec2 = vec1.clone();

        assert_eq!(vec1.get(0), Some(&5));
        assert_eq!(vec2.get(0), Some(&5));
        assert_eq!(vec1.get(1), Some(&10));
        assert_eq!(vec2.get(1), Some(&10));
    }

    #[test]
    fn test_get_out_of_bounds() {
        let mut vec = MyVec::new();
        vec.push(100);

        assert_eq!(vec.get(1), None); // only index 0 is valid
    }

    #[test]
    fn test_empty_pop() {
        let mut vec: MyVec<i32> = MyVec::new();
        assert_eq!(vec.pop(), None);
    }

    #[test]
    fn test_iter() {
        let mut vec = MyVec::new();
        vec.push(10);
        vec.push(20);
        vec.push(30);

        let collected: Vec<_> = (&vec).into_iter().cloned().collect();
        assert_eq!(collected, vec![10, 20, 30]);
    }

    #[test]
    fn test_iter_mut() {
        let mut vec = MyVec::new();
        vec.push(1);
        vec.push(2);
        vec.push(3);

        for x in &mut vec {
            *x *= 2;
        }

        let collected: Vec<_> = (&vec).into_iter().cloned().collect();
        assert_eq!(collected, vec![2, 4, 6]);
    }

    #[test]
    fn test_into_iter() {
        let mut vec = MyVec::new();
        vec.push(5);
        vec.push(6);
        vec.push(7);

        let collected: Vec<_> = vec.into_iter().collect();
        assert_eq!(collected, vec![5, 6, 7]);
    }

    #[test]
    fn test_get_mut() {
        let mut vec = MyVec::new();
        vec.push(5);
        vec.push(6);
        vec.push(7);

        if let Some(val) = vec.get_mut(0) {
            *val += 5;
        }

        assert_eq!(vec.get(0).unwrap(), &10);
    }

    #[test]
    fn test_slice() {
        let mut vec = MyVec::new();
        vec.push(5);
        vec.push(6);
        vec.push(7);

        let vec_slice = vec.as_slice();
        assert_eq!(vec_slice, &[5, 6, 7]);
    }

    #[test]
    fn test_mut_slice() {
        let mut vec = MyVec::new();
        vec.push(5);
        vec.push(6);
        vec.push(7);

        let vec_slice = vec.as_mut_slice();
        assert_eq!(vec_slice, &mut [5, 6, 7]);
    }

    #[test]
    fn test_index() {
        let mut vec = MyVec::new();
        vec.push(5);
        vec.push(6);
        vec.push(7);

        let index_slice = vec.index(0..2);

        assert_eq!(index_slice, &[5, 6]);
    }

    #[test]
    fn test_index_mut() {
        let mut vec = MyVec::new();
        vec.push(5);
        vec.push(6);
        vec.push(7);

        let index_slice = vec.index_mut(0..2);

        assert_eq!(index_slice, &mut [5, 6]);
    }

    #[test]
    fn test_index_inclusive() {
        let mut vec = MyVec::new();
        vec.push(5);
        vec.push(6);
        vec.push(7);

        let index_slice = vec.index(0..=2);

        assert_eq!(index_slice, &[5, 6, 7]);
    }

    #[test]
    fn test_index_and_index_mut() {
        let mut vec = MyVec::new();
        vec.push(10);
        vec.push(20);
        vec.push(30);

        // Test Index (immutable)
        assert_eq!(vec[0], 10);
        assert_eq!(vec[1], 20);
        assert_eq!(vec[2], 30);

        // Test IndexMut (mutable)
        vec[1] = 99;
        assert_eq!(vec[1], 99);

        // Check that other elements are unaffected
        assert_eq!(vec[0], 10);
        assert_eq!(vec[2], 30);
    }

    #[test]
    fn test_insert_middle() {
        let mut vec = MyVec::new();
        vec.push(1);
        vec.push(3);
        vec.insert(1, 2); // Insert 2 between 1 and 3

        assert_eq!(vec.get_len(), 3);
        assert_eq!(vec.as_slice(), &[1, 2, 3]);
    }

    #[test]
    fn test_insert_at_start() {
        let mut vec = MyVec::new();
        vec.push(2);
        vec.push(3);
        vec.insert(0, 1); // Insert at the beginning

        assert_eq!(vec.as_slice(), &[1, 2, 3]);
    }

    #[test]
    fn test_insert_at_end() {
        let mut vec = MyVec::new();
        vec.push(1);
        vec.push(2);
        vec.insert(2, 3); // Insert at the end (like push)

        assert_eq!(vec.as_slice(), &[1, 2, 3]);
    }

    #[test]
    #[should_panic(expected = "assertion failed")]
    fn test_insert_oob() {
        let mut vec = MyVec::new();
        vec.push(1);
        vec.insert(2, 99); // Invalid: index > len
    }

    #[test]
    fn test_remove_middle() {
        let mut vec = MyVec::new();
        vec.push(1);
        vec.push(2);
        vec.push(3);

        let removed = vec.remove(1); // Should remove 2
        assert_eq!(removed, 2);
        assert_eq!(vec.as_slice(), &[1, 3]);
        assert_eq!(vec.get_len(), 2);
    }

    #[test]
    fn test_remove_start() {
        let mut vec = MyVec::new();
        vec.push(1);
        vec.push(2);
        vec.push(3);

        let removed = vec.remove(0); // Should remove 1
        assert_eq!(removed, 1);
        assert_eq!(vec.as_slice(), &[2, 3]);
    }

    #[test]
    fn test_remove_end() {
        let mut vec = MyVec::new();
        vec.push(1);
        vec.push(2);
        vec.push(3);

        let removed = vec.remove(2); // Should remove 3
        assert_eq!(removed, 3);
        assert_eq!(vec.as_slice(), &[1, 2]);
    }

    #[test]
    #[should_panic(expected = "assertion failed")]
    fn test_remove_oob() {
        let mut vec = MyVec::new();
        vec.push(1);
        vec.remove(1); // Invalid: index >= len
    }
}
