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
    /// Any less, and try_get() may start to return None, and get() may spinlock, as all elements may be locked at once.
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
    /// This will not spinlock.
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
    /// This will spinlock if all locks in the pool are taken.
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
    /// If a lock for any of the pooled elements is held elsewhere, then this function will block until
    /// a lock for the given element can be owned by this function.
    pub fn access_all<'a>(&'a self, function: fn(MutexGuard<'a, T>) ) {
        for e in self.elements.iter() {
            // All entries in the pooled try to lock, one at a time, so that the provided function
            // can operate on the pool's contents.
            function(e.lock().unwrap())
        }
    }
}

#[cfg(test)]
mod tests{
    use super::*;

    use std::thread;

    use std::time;

    /// This test can fail, although it is probabilistically unlikely to do so.
    #[test]
    fn counter() {
        let pool: RandomPool<usize> = RandomPool::new(4, || 0);

        for _ in 0..1_000_000 {
            *pool.try_get().unwrap() += 1;
        }
        // Expected value for one counter is 250,000.
        assert!(*pool.try_get().unwrap() > 200_000);
        assert!(*pool.try_get().unwrap() < 300_000);
    }

    /// This test can fail, although it is probabilistically unlikely to do so.
    #[test]
    fn counter_concurrent() {
        // Assign 0 to all 4 initial counters.
        let pool: Arc<RandomPool<usize>> = Arc::new(RandomPool::new(4, || 0));
        let pool_reference_copy_1: Arc<RandomPool<usize>> = pool.clone();
        let pool_reference_copy_2: Arc<RandomPool<usize>> = pool.clone();

        let thread_1 = thread::spawn(move || {
            for _ in 0..500_000 {
                *pool_reference_copy_1.try_get().unwrap() += 1;
            }
        });

        let thread_2 = thread::spawn(move || {
            for _ in 0..500_000 {
                *pool_reference_copy_2.try_get().unwrap() += 1;
            }
        });

        let _ = thread_1.join();
        let _ = thread_2.join();


        // Because both threads add 500,000, split among 4 counters, the expected value for any
        // of the counters is 250,000.
        assert!(*pool.try_get().unwrap() > 200_000);
        assert!(*pool.try_get().unwrap() < 300_000);

    }


    #[test]
    fn alter_all() {
        // Assign 0 to all initial counters
        let pool: RandomPool<usize> = RandomPool::new(4, || 0);

        pool.access_all(|mut x: MutexGuard<usize>| *x = 400 );

        // A `let` binding is needed here to increase the lifetime of the value from the pool.
        let value_from_pool = *pool.try_get().unwrap();
        assert_eq!(value_from_pool, 400);

    }


    #[test]
    fn locks_taken() {
        let pool: Arc<RandomPool<usize>> = Arc::new(RandomPool::new(2, || 7));
        let pool_reference_copy_1: Arc<RandomPool<usize>> = pool.clone();
        let pool_reference_copy_2: Arc<RandomPool<usize>> = pool.clone();

        // Thread 1 owns a lock for 1 second
        let thread_1 = thread::spawn(move || {
            let locked_value = pool_reference_copy_1.try_get().unwrap();
            let one_sec = time::Duration::from_millis(1_000);
            thread::sleep(one_sec);
        });

        // Thread 2 owns a lock for 1 second
        let thread_2 = thread::spawn(move || {
            let locked_value = pool_reference_copy_2.try_get().unwrap();
            let one_sec = time::Duration::from_millis(1_000);
            thread::sleep(one_sec);
        });

        // The main thread waits for half a second, then tries to get a lock.
        let half_a_sec = time::Duration::from_millis(500);
        thread::sleep(half_a_sec);

        // This will fail, because all elements in the pool are locked
        assert!(pool.try_get().is_none())
    }

    #[test]
    fn spinlock() {
        let pool: Arc<RandomPool<usize>> = Arc::new(RandomPool::new(2, || 7));
        let pool_reference_copy_1: Arc<RandomPool<usize>> = pool.clone();
        let pool_reference_copy_2: Arc<RandomPool<usize>> = pool.clone();

        let initial_time = time::Instant::now();
        // Thread 1 owns a lock for 1 second
        let thread_1 = thread::spawn(move || {
            let locked_value = pool_reference_copy_1.try_get().unwrap();
            let one_sec = time::Duration::from_millis(1_000);
            thread::sleep(one_sec);
        });

        // Thread 2 owns a lock for 1 second
        let thread_2 = thread::spawn(move || {
            let locked_value = pool_reference_copy_2.try_get().unwrap();
            let one_sec = time::Duration::from_millis(1_000);
            thread::sleep(one_sec);
        });

        // The main thread waits for half a second, then tries to get a lock.
        // This is to make sure that the earlier threads do get their locks.
        let half_a_sec = time::Duration::from_millis(500);
        thread::sleep(half_a_sec);

        let locked_value = pool.get();

        // even though the `get()` is called after half a second, it must spin for another
        // half a second to wait for one of the threads to unlock one of their locks.
        // When this happens, the spinlock will gain access to the lock, and this assertion can run.
        assert!(initial_time.elapsed() >= time::Duration::from_millis(1_000))


    }

}