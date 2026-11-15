//! Background work for the map: a pool of threads that computes keyed
//! results most urgent first ([`WorkPool`]), and one-off computations
//! ([`Job`]).
//!
//! Both call a `wake` function when a result is ready (the app passes
//! `egui::Context::request_repaint`); nothing here knows about egui, so the
//! queueing rules can be tested on their own.

use std::collections::HashSet;
use std::hash::Hash;
use std::sync::{Arc, Condvar, Mutex, mpsc};
use std::thread;

/// Tells the UI that a result is ready.
pub type Wake = Arc<dyn Fn() + Send + Sync>;

struct Queue<K> {
    /// Requests, most urgent last.
    pending: Vec<K>,
    /// Requests a worker has taken and not finished yet.
    working: HashSet<K>,
    /// The pool is gone: workers stop.
    closed: bool,
}

struct Shared<K> {
    queue: Mutex<Queue<K>>,
    wake: Condvar,
}

/// Threads that compute a `V` for each requested key `K`. Each request
/// replaces the previous one, so work that is no longer wanted is dropped;
/// workers always take the most urgent key left.
pub struct WorkPool<K, V> {
    shared: Arc<Shared<K>>,
    results: mpsc::Receiver<(K, V)>,
    workers: usize,
}

impl<K, V> WorkPool<K, V>
where
    K: Copy + Eq + Hash + Send + 'static,
    V: Send + 'static,
{
    /// Starts `workers` threads named `name-<i>`. Each keeps a state `S`
    /// (for example the world of the last seed) that `compute` may reuse.
    pub fn new<S: Default>(
        name: &str,
        workers: usize,
        wake: Wake,
        compute: impl Fn(&mut S, K) -> V + Send + Sync + 'static,
    ) -> Self {
        let shared = Arc::new(Shared {
            queue: Mutex::new(Queue {
                pending: Vec::new(),
                working: HashSet::new(),
                closed: false,
            }),
            wake: Condvar::new(),
        });
        let (tx, rx) = mpsc::channel();
        let compute = Arc::new(compute);
        let workers = workers.max(1);
        for i in 0..workers {
            let (shared, tx, wake, compute) =
                (shared.clone(), tx.clone(), wake.clone(), compute.clone());
            thread::Builder::new()
                .name(format!("{name}-{i}"))
                .spawn(move || {
                    let mut state = S::default();
                    while let Some(key) = take(&shared) {
                        let value = compute(&mut state, key);
                        // Send before leaving `working`, so the UI never sees
                        // the key as neither done nor in progress.
                        let closed = tx.send((key, value)).is_err();
                        shared
                            .queue
                            .lock()
                            .expect("work queue")
                            .working
                            .remove(&key);
                        if closed {
                            return; // the pool is gone
                        }
                        wake();
                    }
                })
                .expect("spawn worker thread");
        }
        Self {
            shared,
            results: rx,
            workers,
        }
    }

    pub fn workers(&self) -> usize {
        self.workers
    }

    /// Replace the queue with the keys of `wanted` (most urgent first) that
    /// are neither `done` nor being computed. Returns how many keys are
    /// still to come: queued plus in progress.
    pub fn request(&self, wanted: &[K], done: impl Fn(&K) -> bool) -> usize {
        let mut q = self.shared.queue.lock().expect("work queue");
        let mut pending: Vec<K> = wanted
            .iter()
            .filter(|k| !done(k) && !q.working.contains(k))
            .copied()
            .collect();
        let in_flight = pending.len() + q.working.len();
        pending.reverse();
        if q.pending != pending {
            q.pending = pending;
            self.shared.wake.notify_all();
        }
        in_flight
    }

    /// Finished results, in the order they finished.
    pub fn receive(&self) -> impl Iterator<Item = (K, V)> + '_ {
        self.results.try_iter()
    }

    /// Wait for the next result (for tests).
    #[cfg(test)]
    fn wait(&self) -> Option<(K, V)> {
        self.results
            .recv_timeout(std::time::Duration::from_secs(10))
            .ok()
    }
}

/// The next key to compute, or `None` once the pool is gone.
fn take<K: Copy + Eq + Hash>(shared: &Shared<K>) -> Option<K> {
    let mut q = shared.queue.lock().expect("work queue");
    loop {
        if q.closed {
            return None;
        }
        if let Some(key) = q.pending.pop() {
            q.working.insert(key);
            return Some(key);
        }
        q = shared.wake.wait(q).expect("work queue");
    }
}

impl<K, V> Drop for WorkPool<K, V> {
    fn drop(&mut self) {
        // Idle workers stop now; busy ones after their current key.
        if let Ok(mut q) = self.shared.queue.lock() {
            q.closed = true;
            q.pending.clear();
        }
        self.shared.wake.notify_all();
    }
}

/// One value computed on its own thread.
pub struct Job<T> {
    rx: Option<mpsc::Receiver<T>>,
    value: Option<T>,
}

impl<T: Send + 'static> Job<T> {
    pub fn spawn(name: &str, wake: Wake, compute: impl FnOnce() -> T + Send + 'static) -> Self {
        let (tx, rx) = mpsc::channel();
        thread::Builder::new()
            .name(name.into())
            .spawn(move || {
                if tx.send(compute()).is_ok() {
                    wake();
                }
            })
            .expect("spawn job thread");
        Self {
            rx: Some(rx),
            value: None,
        }
    }

    /// Collect the value if it is ready; `true` while it is not.
    pub fn is_loading(&mut self) -> bool {
        if let Some(rx) = &self.rx
            && let Ok(v) = rx.try_recv()
        {
            self.value = Some(v);
            self.rx = None;
        }
        self.rx.is_some()
    }

    pub fn get(&self) -> Option<&T> {
        self.value.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn no_wake() -> Wake {
        Arc::new(|| {})
    }

    /// A pool of one worker whose key 0 blocks until `gate` receives.
    fn gated() -> (WorkPool<u32, u32>, mpsc::Sender<()>) {
        let (gate, gate_rx) = mpsc::channel::<()>();
        let gate_rx = Mutex::new(gate_rx);
        let pool = WorkPool::new("test", 1, no_wake(), move |_: &mut (), k: u32| {
            if k == 0 {
                gate_rx.lock().unwrap().recv().unwrap();
            }
            k * 10
        });
        (pool, gate)
    }

    /// Wait until the single worker has taken key 0.
    fn until_working(pool: &WorkPool<u32, u32>) {
        for _ in 0..1000 {
            if pool.shared.queue.lock().unwrap().working.contains(&0) {
                return;
            }
            thread::sleep(std::time::Duration::from_millis(2));
        }
        panic!("the worker never took key 0");
    }

    #[test]
    fn computes_most_urgent_first() {
        let (pool, gate) = gated();
        assert_eq!(pool.request(&[0], |_| false), 1);
        until_working(&pool);
        // Key 0 is in progress: it is not queued again, and counts once.
        assert_eq!(pool.request(&[0, 1, 2, 3], |_| false), 4);
        gate.send(()).unwrap();
        let order: Vec<(u32, u32)> = (0..4).map(|_| pool.wait().unwrap()).collect();
        assert_eq!(order, [(0, 0), (1, 10), (2, 20), (3, 30)]);
    }

    #[test]
    fn a_new_request_drops_stale_work() {
        let (pool, gate) = gated();
        pool.request(&[0], |_| false);
        until_working(&pool);
        pool.request(&[1, 2], |_| false);
        // The view moved: keys 3 and 4 are wanted now.
        assert_eq!(pool.request(&[3, 4], |_| false), 3);
        gate.send(()).unwrap();
        assert_eq!(pool.wait(), Some((0, 0)));
        assert_eq!(pool.wait(), Some((3, 30)));
        assert_eq!(pool.wait(), Some((4, 40)));
        assert_eq!(pool.request(&[], |_| false), 0);
        assert_eq!(pool.receive().count(), 0);
    }

    #[test]
    fn done_keys_are_not_computed() {
        let pool: WorkPool<u32, u32> = WorkPool::new("test", 2, no_wake(), |_: &mut (), k| k);
        assert_eq!(pool.request(&[1, 2, 3], |&k| k == 2), 2);
        let mut got = vec![pool.wait().unwrap().0, pool.wait().unwrap().0];
        got.sort();
        assert_eq!(got, [1, 3]);
    }

    #[test]
    fn workers_keep_their_state_and_wake_the_ui() {
        let wakes = Arc::new(AtomicUsize::new(0));
        let counter = wakes.clone();
        let pool: WorkPool<u32, u32> = WorkPool::new(
            "test",
            1,
            Arc::new(move || {
                counter.fetch_add(1, Ordering::SeqCst);
            }),
            |calls: &mut u32, _k| {
                *calls += 1;
                *calls
            },
        );
        pool.request(&[7, 8, 9], |_| false);
        let calls: Vec<u32> = (0..3).map(|_| pool.wait().unwrap().1).collect();
        assert_eq!(calls, [1, 2, 3]);
        // The wake comes right after each send.
        for _ in 0..1000 {
            if wakes.load(Ordering::SeqCst) == 3 {
                return;
            }
            thread::sleep(std::time::Duration::from_millis(2));
        }
        panic!("woke {} times", wakes.load(Ordering::SeqCst));
    }

    #[test]
    fn dropping_the_pool_stops_idle_workers() {
        // Each worker holds the compute closure, and with it this token.
        let token = Arc::new(());
        let held = token.clone();
        let pool: WorkPool<u32, u32> = WorkPool::new("test", 3, no_wake(), move |_: &mut (), k| {
            let _ = &held;
            k
        });
        assert!(Arc::strong_count(&token) > 1);
        drop(pool);
        for _ in 0..1000 {
            if Arc::strong_count(&token) == 1 {
                return;
            }
            thread::sleep(std::time::Duration::from_millis(2));
        }
        panic!("the workers kept running");
    }

    #[test]
    fn job_delivers_once() {
        let mut job = Job::spawn("test", no_wake(), || 42);
        for _ in 0..1000 {
            if !job.is_loading() {
                assert_eq!(job.get(), Some(&42));
                assert!(!job.is_loading());
                return;
            }
            thread::sleep(std::time::Duration::from_millis(2));
        }
        panic!("the job never finished");
    }
}
