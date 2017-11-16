# Random Pool
A threadsafe, fixed-size, persistent object pool,
where its contents are individually guarded by Mutexes 
and are guaranteed to be accessed in a random order.

# Use Case 
This crate is useful for situations where you need a fixed number of
mutable elements that can be shared across threads,
but the particular element you are accessing is not important.

This is useful either when you want a pool of identical resources
that you don't intend to modify, like DB connections,
or when you want a set of resources that when accessed randomly,
will trend towards having the same contents, like a dynamic cache. 


# Features
* Threadsafe - The pool can be shared across threads if wrapped in an `Arc`.
* Interior mutability - Elements in the pool can be mutated.
* Random access - An element in the pool will be randomly returned to the caller if it is not already locked.
* Get elements by either possibly waiting on a spinlock to release ( `get()` ),
 or to return None if all elements are currently locked ( `try_get()` ). 
* Run a custom function on all elements in the pool, one element at a time.
