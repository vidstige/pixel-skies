//! Order-preserving parallel map over an index range, on std threads.

use std::thread;

/// Chunks per core. More, smaller chunks let fast cores pick up the work
/// slow ones would otherwise straggle on.
const CHUNKS_PER_THREAD: usize = 8;

pub fn map<T: Send>(n: usize, f: impl Fn(usize) -> T + Sync) -> Vec<T> {
    let threads = thread::available_parallelism().map_or(1, |t| t.get());
    let chunk = n.div_ceil(threads * CHUNKS_PER_THREAD);
    let f = &f;
    thread::scope(|scope| {
        let handles: Vec<_> = (0..n)
            .step_by(chunk)
            .map(|start| {
                let range = start..n.min(start + chunk);
                scope.spawn(move || range.map(f).collect::<Vec<T>>())
            })
            .collect();
        handles.into_iter().flat_map(|h| h.join().unwrap()).collect()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn results_come_back_in_index_order() {
        let squares: Vec<usize> = (0..1000).map(|i| i * i).collect();
        assert_eq!(map(1000, |i| i * i), squares);
    }
}
