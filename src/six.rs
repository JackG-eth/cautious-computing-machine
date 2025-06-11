use std::{marker::PhantomData, ptr::null_mut};


pub struct List<T> {
    head: Link<T>,
    tail: *mut Node<T>,
}

type Link<T> = *mut Node<T>; // MUCH BETTER

struct Node<T> {
    elem: T,
    next: Link<T>,
}

pub struct IntoIter<T>(List<T>);

pub struct Iter<'a, T> {
    next: Option<&'a Node<T>>,
}

pub struct IterMut<'a, T> {
    next: Option<&'a mut Node<T>>,
}

impl<T> List<T> {
    fn new() -> Self {
        Self {
            head: null_mut(),
            tail: null_mut(),
        }
    }

    // Pushes a new element onto the end of the list.
    fn push(&mut self, elem: T) {
        unsafe {
            // Allocate a new node on the heap and get a raw pointer to it.
            // `Box::new` puts it on the heap, `Box::into_raw` gives up ownership and turns it into a raw pointer.
            let new_tail = Box::into_raw(Box::new(Node {
                elem,
                next: null_mut(), // New node has no next yet; it's the end.
            }));

            // If the list isn't empty (i.e., tail is non-null), link the current tail to the new node.
            if !self.tail.is_null() {
                // Update the current tail's next pointer to point to the new node.
                (*self.tail).next = new_tail;
            } else {
                // If the list was empty, then this new node is also the head.
                self.head = new_tail;
            }

            // In both cases, move the tail pointer to the new node.
            self.tail = new_tail;
        }
    }

    // Removes and returns the element from the front of the list, if it exists.
    pub fn pop(&mut self) -> Option<T> {
        unsafe {
            if self.head.is_null() {
                // The list is empty, nothing to pop.
                None
            } else {
                // Take ownership of the head node by reconstructing the Box.
                // This reclaims the memory and lets Rust drop the node safely.
                let head = Box::from_raw(self.head);

                // Move the head pointer to the next node in the list.
                self.head = head.next;

                // If the list is now empty, also nullify the tail.
                if self.head.is_null() {
                    self.tail = null_mut();
                }

                // Return the element of the old head.
                Some(head.elem)
            }
        }
    }  

    pub fn into_iter(self) -> IntoIter<T> {
        IntoIter(self)
    }

    pub fn iter(&self) -> Iter<'_, T> {
        unsafe {
            Iter { next: self.head.as_ref() }
        }
    }

    pub fn iter_mut(&mut self) -> IterMut<'_, T> {
        unsafe {
            IterMut { next: self.head.as_mut() }
        }
    }

    pub fn peek(&self) -> Option<&T> {
        unsafe {
          self.head.as_ref().map(|node| &node.elem)
        }
    }
    
    pub fn peek_mut(&mut self) -> Option<&mut T> {
        unsafe {
            self.head.as_mut().map(|node| &mut node.elem)
        }
    }
}

impl<T> Drop for List<T> {
    fn drop(&mut self) {
        while let Some(_) = self.pop() { }
    }
}

impl<T> Iterator for IntoIter<T> {
    type Item = T;
    fn next(&mut self) -> Option<Self::Item> {
        self.0.pop()
    }
}

impl<'a, T> Iterator for Iter<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
       unsafe {
            self.next.map(|node| {
                self.next = node.next.as_ref();
                &node.elem
            })
       }
    }
}

impl<'a, T> Iterator for IterMut<'a, T> {
    type Item = &'a mut T;

    fn next(&mut self) -> Option<Self::Item> {
        unsafe {
            self.next.take().map(|node| {
                self.next = node.next.as_mut();
                &mut node.elem
            })
       }
    }
}



#[cfg(test)]
mod test {
    use crate::six::List;

    
    #[cfg(test)]
    mod test {
        use super::List;
        #[test]
        fn basics() {
            let mut list = List::new();
    
            // Check empty list behaves right
            assert_eq!(list.pop(), None);
    
            // Populate list
            list.push(1);
            list.push(2);
            list.push(3);
    
            // Check normal removal
            assert_eq!(list.pop(), Some(1));
            assert_eq!(list.pop(), Some(2));
    
            // Push some more just to make sure nothing's corrupted
            list.push(4);
            list.push(5);
    
            // Check normal removal
            assert_eq!(list.pop(), Some(3));
            assert_eq!(list.pop(), Some(4));
    
            // Check exhaustion
            assert_eq!(list.pop(), Some(5));
            assert_eq!(list.pop(), None);
    
            // Check the exhaustion case fixed the pointer right
            list.push(6);
            list.push(7);
    
            // Check normal removal
            assert_eq!(list.pop(), Some(6));
            assert_eq!(list.pop(), Some(7));
            assert_eq!(list.pop(), None);
        }
    }
    
}
