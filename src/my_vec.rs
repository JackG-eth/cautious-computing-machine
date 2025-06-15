use std::{
    alloc::{self, Layout},
    mem::MaybeUninit,
    ptr::{self, NonNull},
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

    fn get(&self, index: usize) -> Option<T> {
        if index >= self.len {
            None
        } else {
            Some(self.data.read(index))
        }
    }

    fn get_len(&self) -> usize {
        self.len
    }

    fn get_capacity(&self) -> usize {
        self.data.cap
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
            let new_ptr = alloc::realloc(
                self.ptr.as_ptr() as *mut u8,
                old_layout,
                new_layout.size(),
            ) as *mut MaybeUninit<T>;

            self.ptr = NonNull::new(new_ptr)
                .unwrap_or_else(|| alloc::handle_alloc_error(new_layout));
        }

        self.cap = new_cap;
    }

    fn write(&mut self, index: usize, value: T) {
        unsafe {
            (*self.ptr.as_ptr().add(index)).write(value);
        }
    }

    fn read_last(&self, len: usize) -> T {
        unsafe { ptr::read((*self.ptr.as_ptr().add(len)).as_ptr()) }
    }

    fn read(&self, index: usize) -> T {
        unsafe { ptr::read((*self.ptr.as_ptr().add(index)).as_ptr()) }
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

impl<T> Clone for RawVec<T> {
    fn clone(&self) -> Self {
        Self {
            ptr: self.ptr,
            cap: self.cap,
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

        assert_eq!(vec.get(0), Some(10));
        assert_eq!(vec.get(1), Some(20));
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

        assert_eq!(vec1.get(0), Some(5));
        assert_eq!(vec2.get(0), Some(5));
        assert_eq!(vec1.get(1), Some(10));
        assert_eq!(vec2.get(1), Some(10));
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
}
