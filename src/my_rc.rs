/*
    counts how many references
    doesnt doesn't drop unless the count == 1 (use atomics for this?)
    mutability if count == 1?
*/

use std::{
    cell::{Cell, RefCell},
    ops::{Deref, DerefMut},
    ptr::NonNull,
};

pub struct InnerRc<T> {
    value: T,
    count: Cell<usize>,
}

impl<T> InnerRc<T> {
    pub fn new(value: T) -> Self {
        Self {
            value,
            count: Cell::new(1),
        }
    }

    pub fn get_count(&self) -> usize {
        self.count.get()
    }

    pub fn get_ref(&self) -> &T {
        &self.value
    }
}

struct MyRc<T> {
    ptr: NonNull<InnerRc<T>>,
}

impl<T> MyRc<T> {
    fn new(value: T) -> Self {
        let inner_ptr =
            unsafe { NonNull::new_unchecked(Box::into_raw(Box::new(InnerRc::new(value)))) };
        Self { ptr: inner_ptr }
    }

    fn try_unwrap(self) -> Result<T, Self> {
        if self.get_count() == 1 {
            let inner = unsafe { Box::from_raw(self.ptr.as_ptr()) };
            let value = inner.value;
            std::mem::forget(self); // prevent drop
            Ok(value)
        } else {
            Err(self)
        }
    }

    fn get_count(&self) -> usize {
        unsafe { (*self.ptr.as_ptr()).count.get() }
    }

    fn get_value_ref(&self) -> &T {
        unsafe { &(*self.ptr.as_ptr()).value }
    }

    pub fn get_mut_ref(&mut self) -> Option<&mut T> {
        unsafe {
            let inner = self.ptr.as_ref();
            if inner.count.get() == 1 {
                Some(&mut (*self.ptr.as_mut()).value)
            } else {
                None
            }
        }
    }
}

impl<T> Clone for MyRc<T> {
    fn clone(&self) -> Self {
        unsafe {
            let inner = self.ptr.as_ref();
            inner.count.set(inner.count.get() + 1);
        }
        Self { ptr: self.ptr }
    }
}

impl<T> Deref for MyRc<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &(*self.ptr.as_ref()).value }
    }
}

impl<T> Drop for MyRc<T> {
    fn drop(&mut self) {
        unsafe {
            let inner = self.ptr.as_ref();

            if inner.count.get() == 0 {
                panic!("Double drop detected!");
            }

            if inner.count.get() != 1 {
                inner.count.set(inner.count.get() - 1);
            } else {
                drop(Box::from_raw(self.ptr.as_ptr()));
                println!("Dropped MyRc");
            }
        }
    }
}

#[cfg(test)]
pub mod test {
    use super::MyRc;

    #[test]
    fn test_basics() {
        let my_rc = MyRc::new("bob");

        assert_eq!(my_rc.get_count(), 1);

        let mc_clone = my_rc.clone();

        assert_eq!(mc_clone.get_count(), 2);

        assert_eq!(mc_clone.get_value_ref(), &"bob");
    }

    #[test]
    fn test_drop_behavior() {
        struct Tracker<'a>(&'a str);

        impl<'a> Drop for Tracker<'a> {
            fn drop(&mut self) {
                println!("Dropped Tracker({})", self.0);
            }
        }

        {
            let a = MyRc::new(Tracker("a"));

            let b = a.clone();
            let c = b.clone();
            assert_eq!(a.get_count(), 3);
            // all dropped here, should print once
        }

        // You should see "Dropped Tracker(a)" once in output
    }

    #[test]
    fn test_get_mut_ref_only_when_unique() {
        let mut rc = MyRc::new(42);
        assert_eq!(rc.get_count(), 1);

        {
            // Should succeed
            let value = rc.get_mut_ref();
            assert!(value.is_some());
            *value.unwrap() = 100;
        }

        let rc2 = rc.clone();
        assert_eq!(rc.get_count(), 2);

        // Should fail now
        assert!(rc.get_mut_ref().is_none());

        assert_eq!(rc.get_value_ref(), &100);
    }

    #[test]
    fn test_deref() {
        let rc = MyRc::new(String::from("hello"));
        assert_eq!(rc.len(), 5); // using Deref to String
    }
}
