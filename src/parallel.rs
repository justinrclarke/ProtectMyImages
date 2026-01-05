//! Parallel processing utilities.
//!
//! Provides a thread pool for parallel file processing using only the standard library.

use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

/// A job to be executed by the thread pool.
type Job = Box<dyn FnOnce() + Send + 'static>;

/// A simple thread pool for parallel task execution.
pub struct ThreadPool {
    workers: Vec<Worker>,
    sender: Option<Sender<Job>>,
}

impl ThreadPool {
    /// Create a new thread pool with the specified number of threads.
    ///
    /// # Panics
    /// Panics if `num_threads` is 0.
    pub fn new(num_threads: usize) -> Self {
        assert!(num_threads > 0, "Thread pool must have at least 1 thread");

        let (sender, receiver) = mpsc::channel();
        let receiver = Arc::new(Mutex::new(receiver));

        let mut workers = Vec::with_capacity(num_threads);
        for id in 0..num_threads {
            workers.push(Worker::new(id, Arc::clone(&receiver)));
        }

        ThreadPool {
            workers,
            sender: Some(sender),
        }
    }

    /// Create a thread pool with the optimal number of threads for this system.
    pub fn optimal() -> Self {
        let num_cpus = available_parallelism();
        Self::new(num_cpus)
    }

    /// Execute a job on the thread pool.
    pub fn execute<F>(&self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        let job = Box::new(f);
        if let Some(ref sender) = self.sender {
            sender.send(job).expect("Failed to send job to thread pool");
        }
    }

    /// Get the number of worker threads.
    pub fn num_threads(&self) -> usize {
        self.workers.len()
    }
}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        // Drop the sender to signal workers to stop.
        drop(self.sender.take());

        // Wait for all workers to finish.
        for worker in &mut self.workers {
            if let Some(thread) = worker.thread.take() {
                let _ = thread.join();
            }
        }
    }
}

/// A worker thread in the pool.
struct Worker {
    _id: usize,
    thread: Option<JoinHandle<()>>,
}

impl Worker {
    fn new(id: usize, receiver: Arc<Mutex<Receiver<Job>>>) -> Self {
        let thread = thread::spawn(move || {
            loop {
                let job = {
                    let lock = receiver.lock().expect("Worker mutex poisoned");
                    lock.recv()
                };

                match job {
                    Ok(job) => job(),
                    Err(_) => break, // Channel closed, exit the loop.
                }
            }
        });

        Worker {
            _id: id,
            thread: Some(thread),
        }
    }
}

/// Get the number of available CPU cores.
pub fn available_parallelism() -> usize {
    thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
}

/// Parallel map operation over a collection.
///
/// Processes items in parallel using a thread pool and collects results.
pub fn parallel_map<T, R, F>(items: Vec<T>, f: F, num_threads: usize) -> Vec<R>
where
    T: Send + 'static,
    R: Send + 'static,
    F: Fn(T) -> R + Send + Sync + 'static,
{
    if items.is_empty() {
        return Vec::new();
    }

    // For small workloads, just process sequentially.
    if items.len() <= 2 || num_threads <= 1 {
        return items.into_iter().map(f).collect();
    }

    let num_threads = num_threads.min(items.len());
    let item_count = items.len();
    let f = Arc::new(f);

    // Use a Vec that we'll populate with results indexed by position
    let results: Arc<Mutex<Vec<(usize, R)>>> = Arc::new(Mutex::new(Vec::with_capacity(item_count)));

    let pool = ThreadPool::new(num_threads);

    for (index, item) in items.into_iter().enumerate() {
        let f = Arc::clone(&f);
        let results = Arc::clone(&results);

        pool.execute(move || {
            let result = f(item);
            let mut results = results.lock().expect("Results mutex poisoned");
            results.push((index, result));
        });
    }

    // Drop pool to wait for all jobs to complete.
    drop(pool);

    // Extract results and sort by original index to maintain order.
    let mut results = match Arc::try_unwrap(results) {
        Ok(mutex) => mutex.into_inner().expect("Results mutex poisoned"),
        Err(_) => panic!("Results Arc still has references"),
    };
    results.sort_by_key(|(idx, _)| *idx);
    results.into_iter().map(|(_, r)| r).collect()
}

/// Process items in parallel, calling a callback for each result.
///
/// This is useful when you want to process results as they complete
/// rather than waiting for all results.
pub fn parallel_for_each<T, F, C>(items: Vec<T>, process: F, mut callback: C, num_threads: usize)
where
    T: Send + 'static,
    F: Fn(T) + Send + Sync + 'static,
    C: FnMut(),
{
    if items.is_empty() {
        return;
    }

    if items.len() <= 2 || num_threads <= 1 {
        for item in items {
            process(item);
            callback();
        }
        return;
    }

    let num_threads = num_threads.min(items.len());
    let process = Arc::new(process);
    let (done_tx, done_rx) = mpsc::channel::<()>();
    let total = items.len();

    let pool = ThreadPool::new(num_threads);

    for item in items {
        let process = Arc::clone(&process);
        let done_tx = done_tx.clone();

        pool.execute(move || {
            process(item);
            let _ = done_tx.send(());
        });
    }

    // Drop our sender so the channel closes when all workers are done.
    drop(done_tx);

    // Process callbacks as items complete.
    let mut completed = 0;
    while completed < total {
        if done_rx.recv().is_ok() {
            completed += 1;
            callback();
        } else {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn test_available_parallelism() {
        let cpus = available_parallelism();
        assert!(cpus >= 1);
    }

    #[test]
    fn test_thread_pool_creation() {
        let pool = ThreadPool::new(4);
        assert_eq!(pool.num_threads(), 4);
    }

    #[test]
    fn test_thread_pool_optimal() {
        let pool = ThreadPool::optimal();
        assert!(pool.num_threads() >= 1);
    }

    #[test]
    fn test_thread_pool_execute() {
        let counter = Arc::new(AtomicUsize::new(0));
        let pool = ThreadPool::new(2);

        for _ in 0..10 {
            let counter = Arc::clone(&counter);
            pool.execute(move || {
                counter.fetch_add(1, Ordering::SeqCst);
            });
        }

        drop(pool); // Wait for completion.
        assert_eq!(counter.load(Ordering::SeqCst), 10);
    }

    #[test]
    fn test_parallel_map() {
        let items: Vec<i32> = (1..=10).collect();
        let results = parallel_map(items, |x| x * 2, 4);

        assert_eq!(results.len(), 10);
        for (i, &r) in results.iter().enumerate() {
            assert_eq!(r, ((i + 1) * 2) as i32);
        }
    }

    #[test]
    fn test_parallel_map_empty() {
        let items: Vec<i32> = vec![];
        let results = parallel_map(items, |x| x * 2, 4);
        assert!(results.is_empty());
    }

    #[test]
    fn test_parallel_map_single_thread() {
        let items: Vec<i32> = (1..=5).collect();
        let results = parallel_map(items, |x| x + 1, 1);

        assert_eq!(results, vec![2, 3, 4, 5, 6]);
    }

    #[test]
    fn test_parallel_for_each() {
        let items: Vec<i32> = (1..=10).collect();
        let processed = Arc::new(AtomicUsize::new(0));
        let callbacks = Arc::new(AtomicUsize::new(0));

        let processed_clone = Arc::clone(&processed);
        let callbacks_clone = Arc::clone(&callbacks);

        parallel_for_each(
            items,
            move |_| {
                processed_clone.fetch_add(1, Ordering::SeqCst);
            },
            move || {
                callbacks_clone.fetch_add(1, Ordering::SeqCst);
            },
            4,
        );

        assert_eq!(processed.load(Ordering::SeqCst), 10);
        assert_eq!(callbacks.load(Ordering::SeqCst), 10);
    }

    #[test]
    #[should_panic(expected = "Thread pool must have at least 1 thread")]
    fn test_thread_pool_zero_threads() {
        let _ = ThreadPool::new(0);
    }
}
