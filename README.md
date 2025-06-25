# Raw Folder: Low-Level Rust Experiments

This folder contains my experiments and learning projects for understanding low-level Rust programming. The goal is to deepen my understanding of how the Rust standard library works under the hood, especially with regard to memory management, data structures, and safe(ish) abstractions over unsafe code.

## Why?
- To learn how collections like Vec, VecDeque, and others are implemented.
- To get hands-on experience with manual memory management, pointer arithmetic and much more....
- To build intuition for how Rust achieves safety and performance.
- To become a better Rust programmer by understanding the "raw" side of things.

## What's in this folder?

- **raw_vec.rs**: My own implementation of a low-level, growable vector buffer (like Vec<T>'s internal buffer). Handles allocation, reallocation, and deallocation. Used as a building block for higher-level collections.

- **raw_deque.rs**: A double-ended queue (deque) built on top of a raw buffer. Supports pushing and popping from both ends, with circular buffer logic and custom iterators. Mimics the behavior of VecDeque<T>.

- **my_arc.rs**: A simple version of Arc with weak and strong refs.

- **my_rc.rs**: A simple version of Rc.

- **my_linked_list.rs**: My own implementation of a low-level, growable double-ended linked list.

-- TODO will be adding more slowly..

**Feel free to point out any glaring issues!** 