use std::{
    ops::Deref,
    ptr::NonNull,
    rc::Weak,
    sync::atomic::{AtomicUsize, Ordering, fence},
};

pub struct InnerArc<T> {
    value: T,
    strong: AtomicUsize,
    weak: AtomicUsize,
}

impl<T> InnerArc<T> {
    fn new(value: T) -> Self {
        InnerArc {
            value,
            strong: AtomicUsize::new(1),
            weak: AtomicUsize::new(1), // start at 1 because Arc holds a weak ref to itself internally
        }
    }
}

pub struct MyArc<T> {
    ptr: NonNull<InnerArc<T>>,
}

pub struct MyWeak<T> {
    ptr: NonNull<InnerArc<T>>,
}

impl<T> MyWeak<T> {
    fn upgrade(&self) -> Option<MyArc<T>> {
        unsafe {
            let inner = self.ptr.as_ref();
            let mut strong_count = inner.strong.load(Ordering::Acquire);

            while strong_count != 0 {
                match inner.strong.compare_exchange(
                    strong_count,
                    strong_count + 1,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                ) {
                    Ok(_) => return Some(MyArc { ptr: self.ptr }),
                    Err(updated) => strong_count = updated,
                }
            }
            None
        }
    }
}

impl<T> MyArc<T> {
    fn new(value: T) -> Self {
        let ptr = unsafe { NonNull::new_unchecked(Box::into_raw(Box::new(InnerArc::new(value)))) };
        MyArc { ptr }
    }

    fn get_strong_count(&self) -> usize {
        unsafe {
            (*self.ptr.as_ref())
                .strong
                .load(std::sync::atomic::Ordering::SeqCst)
        }
    }

    fn get_weak_count(&self) -> usize {
        unsafe {
            (*self.ptr.as_ref())
                .weak
                .load(std::sync::atomic::Ordering::SeqCst)
        }
    }

    // todo might have to check weak as well, although it would have to upgrade?
    fn get_mut_ref(&mut self) -> Option<&mut T> {
        unsafe {
            let inner_ptr = &(*self.ptr.as_ref());
            if inner_ptr.strong.load(std::sync::atomic::Ordering::SeqCst) == 1 {
                return Some(&mut (*self.ptr.as_mut()).value);
            }
            None
        }
    }

    fn get_value_ref(&self) -> &T {
        unsafe { &(*self.ptr.as_ref()).value }
    }

    fn try_unwrap(self) -> Result<T, Self> {
        if self.get_strong_count() == 1 {
            let unboxed = unsafe { Box::from_raw(self.ptr.as_ptr()) };
            let value = unboxed.value;
            std::mem::forget(self); // prevent drop
            Ok(value)
        } else {
            Err(self)
        }
    }

    // do we not need to dec strong count
    fn downgrade(&self) -> MyWeak<T> {
        unsafe {
            let inner = self.ptr.as_ref();
            // Increment weak count
            inner.weak.fetch_add(1, Ordering::Relaxed);
            MyWeak { ptr: self.ptr }
        }
    }
}

impl<T> Clone for MyArc<T> {
    fn clone(&self) -> Self {
        unsafe {
            (*self.ptr.as_ref())
                .strong
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
        Self { ptr: self.ptr }
    }
}

impl<T> Clone for MyWeak<T> {
    fn clone(&self) -> Self {
        unsafe {
            (*self.ptr.as_ref()).weak.fetch_add(1, Ordering::Relaxed);
        }
        Self { ptr: self.ptr }
    }
}

impl<T> Drop for MyWeak<T> {
    fn drop(&mut self) {
        unsafe {
            let inner = self.ptr.as_ref();
            if inner.weak.fetch_sub(1, Ordering::Release) == 1 {
                fence(Ordering::Acquire);
                drop(Box::from_raw(self.ptr.as_ptr()));
            }
        }
    }
}

impl<T> Deref for MyArc<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &(*self.ptr.as_ref()).value }
    }
}

impl<T> Drop for MyArc<T> {
    fn drop(&mut self) {
        unsafe {
            let inner = self.ptr.as_ref();
            if inner
                .strong
                .fetch_sub(1, std::sync::atomic::Ordering::Release)
                != 1
            {
                return;
            }
            drop(Box::from_raw(self.ptr.as_ptr()));

            // Now decrement weak count because the Arc itself holds a weak ref
            if inner.weak.fetch_sub(1, Ordering::Release) == 1 {
                std::sync::atomic::fence(Ordering::Acquire);
                // finally deallocate InnerArc
                drop(Box::from_raw(self.ptr.as_ptr()));
            }
        }
    }
}

unsafe impl<T> Send for MyArc<T> {}
unsafe impl<T> Sync for MyArc<T> {}

#[cfg(test)]
pub mod test {
    use std::{ops::Deref, sync::Mutex, thread};

    use super::MyArc;

    #[test]
    fn test_multithreaded_ref_counting() {
        let arc = MyArc::new(Mutex::new(0));

        let initial_count = arc.get_strong_count();
        assert_eq!(initial_count, 1);

        let mut handles = vec![];

        for _ in 0..10 {
            let arc_clone = arc.clone(); // count += 1
            handles.push(thread::spawn(move || {
                let mut lock = arc_clone.deref().lock().unwrap();
                *lock += 1;
                // Drop happens automatically when arc_clone goes out of scope
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // After all threads finish, only `arc` should be left
        assert_eq!(arc.get_strong_count(), 1);

        let final_value = arc.deref().lock().unwrap();
        assert_eq!(*final_value, 10);
    }

    #[test]
    fn test_try_unwrap_success() {
        let rc = MyArc::new(String::from("own me"));
        let unwrapped = rc.try_unwrap().unwrap_or("test".to_string());
        assert_eq!(unwrapped, "own me");
    }

    #[test]
    fn test_try_unwrap_fail() {
        let rc = MyArc::new(String::from("shared"));
        let _clone = rc.clone();
        let result = rc.try_unwrap();
        assert!(result.is_err());
    }

    #[test]
    fn test_get_mut_ref_arc() {
        let mut rc = MyArc::new(123);
        assert!(rc.get_mut_ref().is_some());

        let _clone = rc.clone();
        assert!(rc.get_mut_ref().is_none());
    }

    #[test]
    fn test_basics() {
        let my_rc = MyArc::new("bob");

        assert_eq!(my_rc.get_strong_count(), 1);

        let mc_clone = my_rc.clone();

        assert_eq!(mc_clone.get_strong_count(), 2);

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
            let a = MyArc::new(Tracker("a"));

            let b = a.clone();
            let c = b.clone();
            assert_eq!(a.get_strong_count(), 3);
            // all dropped here, should print once
        }

        // You should see "Dropped Tracker(a)" once in output
    }

    #[test]
    fn test_get_mut_ref_only_when_unique() {
        let mut rc = MyArc::new(42);
        assert_eq!(rc.get_strong_count(), 1);

        {
            // Should succeed
            let value = rc.get_mut_ref();
            assert!(value.is_some());
            *value.unwrap() = 100;
        }

        let rc2 = rc.clone();
        assert_eq!(rc.get_strong_count(), 2);

        // Should fail now
        assert!(rc.get_mut_ref().is_none());

        assert_eq!(rc.get_value_ref(), &100);
    }

    #[test]
    fn test_deref() {
        let rc = MyArc::new(String::from("hello"));
        assert_eq!(rc.len(), 5); // using Deref to String
    }

    #[test]
    fn test_weak_upgrade_success() {
        let arc = MyArc::new(42);
        let weak = arc.downgrade();

        assert_eq!(arc.get_strong_count(), 1);
        assert_eq!(arc.get_weak_count(), 2); // arc holds 1 implicit weak, and we created another

        let upgraded = weak.upgrade();
        assert!(upgraded.is_some());
        assert_eq!(*upgraded.unwrap(), 42);
    }

    #[test]
    fn test_weak_upgrade_fails_after_arc_drop() {
        let weak = {
            let arc = MyArc::new("hello".to_string());
            let weak = arc.downgrade();
            assert!(weak.upgrade().is_some());
            weak
        };

        // All strong references dropped now
        assert!(weak.upgrade().is_none());
    }

    #[test]
    fn test_weak_counts() {
        let arc = MyArc::new("hi");
        assert_eq!(arc.get_weak_count(), 1); // internal weak ref

        let w1 = arc.downgrade();
        assert_eq!(arc.get_weak_count(), 2);

        let w2 = w1.clone();
        assert_eq!(arc.get_weak_count(), 3);

        drop(w1);
        assert_eq!(arc.get_weak_count(), 2);

        drop(w2);
        assert_eq!(arc.get_weak_count(), 1); // back to implicit only
    }
}
