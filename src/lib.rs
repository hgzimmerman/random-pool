extern crate rand;

use rand::{thread_rng, Rng};

use std::sync::Arc;
use std::sync::{Mutex, MutexGuard};

/// A threadsafe pool that holds elements that are each individually guarded behind a Mutex.
///
/// When getting an element, a random element is selected from the pool, locked, and returned.
/// If a lock for the random element cannot be gotten, the pool will try the next available element.
#[derive(Clone, Debug)]
struct RandomPool<T> {
    elements: Vec<Arc<Mutex<T>>>
}

impl<T> RandomPool<T> {

    /// Create a new pool.
    /// You should want the number of entries to correspond to the number of threads that may access the pool.
    /// Any more, and you are wasting space for elements that won't relieve lock contention.
    /// Any less, and you can't safely unwrap() the result of try_get(), as all elements may be locked at once.
    pub fn new(number_of_entries: usize, element_creation_function: fn() -> T) -> RandomPool<T> {
        let mut elements: Vec<Arc<Mutex<T>>> = vec!();

        for _ in 0..number_of_entries {
            elements.push(Arc::new(Mutex::new(element_creation_function())))
        }
        RandomPool {
            elements: elements
        }
    }


    /// Try to get a random element from the pool.
    /// If all elements are locked, this will return `None`.
    ///
    /// # Warning
    ///
    /// This will not deadlock.
    ///
    /// It is possible for this to miss an unlocked element if an element that has been passed over
    /// because it was locked, becomes unlocked after it was checked, but before the method ends.
    ///
    /// Despite how rare this event is, it is unwise to call `unwrap()` on the Option returned
    /// from this function, as this may return `None` because of this concurrency quirk.
    pub fn try_get<'a>(&'a self) -> Option<MutexGuard<'a, T>> {

        // Randomize the range that can be accessed
        let mut range: Vec<usize> = (0..self.elements.len()).collect();
        thread_rng().shuffle(range.as_mut_slice());

        for i in range.into_iter() {
            if let Some(c) = self.elements[i].try_lock().ok() {
                println!("Locked element at index: {}", i);
                return Some(c) // Found a cache that wasn't locked
            }
        }
        None // All caches are occupied
    }

    /// Attempts to return a random element from the pool.
    /// If the first element is locked, it will try the next random element.
    /// If all elements are locked, the pool will deadlock until one of the locks frees itself.
    ///
    /// # Warning
    ///
    /// This has the possibility to deadlock if all locks are owned and never independently unlock themselves.
    pub fn get<'a>(&'a self) -> MutexGuard<'a, T> {
        // Randomize the range that can be accessed
        let mut range: Vec<usize> = (0..self.elements.len()).collect();
        thread_rng().shuffle(range.as_mut_slice());

        let mut index: usize = 0;
        loop {
            match self.elements[index].try_lock().ok() {
                Some(element) => return element,
                None => index = (index + 1) % self.elements.len()
            }
        }
    }


    /// Alter every element in the pool by locking them one at a time.
    ///
    /// # Warning
    ///
    /// If a lock for one of the pooled elements is held elsewhere, and the provided function
    /// would cause that element to remain locked, this has the possibility to deadlock.
    pub fn access_all<'a>(&'a self, function: fn(MutexGuard<'a, T>) ) {
        for e in self.elements.iter() {
            // All entries in the pooled try to lock, one at a time, so that the provided function
            // can operate on the pool's contents.
            function(e.lock().unwrap())
        }
    }
}