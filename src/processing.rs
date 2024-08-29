use futures::{
    stream::{self, StreamExt},
    Stream,
};
use indicatif::{ProgressBar, ProgressStyle};
use log::error;
use std::{
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};
use tokio::task;

use std::sync::atomic::AtomicUsize;
use std::sync::Arc;

/// Consume a stream of items concurrently into a function
/// Args:
/// iter: An iterator of items to process
/// f: A function that processes an item
/// progress: Whether to show a progress bar
/// concurrency_opt: The number of concurrent tasks to run
/// Returns: None
///
/// Example:
/// async fn process_item(item: i32) {
///     // Simulate some async work
///     tokio::time::sleep(std::time::Duration::from_millis(10)).await;
///     println!("Processed item: {}", item);
/// }
///
/// async fn main() {
///    let items = vec![1, 2, 3, 4, 5];
/// >  consume_concurrently(items, process_item, false, None).await;
/// }
pub async fn consume_concurrently<I, T, F, C, Fut>(
    iter: I,
    f: F,
    context: &C,
    progress: bool,
    concurrency_opt: Option<usize>,
) where
    I: IntoIterator<Item = T>,
    T: Send + 'static,
    F: Fn(T, C) -> Fut + Clone + Send + 'static,
    Fut: std::future::Future<Output = ()> + Send + 'static,
    C: Clone + Send + 'static,
{
    let concurrency: usize;
    if let Some(conc) = concurrency_opt {
        concurrency = conc;
    } else {
        concurrency = std::thread::available_parallelism().unwrap().get();
    }
    let tstream = stream::iter(iter);
    let (_, upper) = tstream.size_hint();
    let progress_bar: Option<ProgressBar>;
    if progress {
        if upper.is_some() {
            progress_bar = Some(ProgressBar::new(upper.unwrap() as u64));
            progress_bar.as_ref().unwrap().set_style(
                ProgressStyle::default_bar()
                    .template(
                        "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})",
                    )
                    .expect("Progress template creation failed")
                    .progress_chars("#>-"),
            );
        } else {
            progress_bar = Some(ProgressBar::new_spinner());

            // Set a custom style if desired
            progress_bar.as_ref().unwrap().set_style(
                ProgressStyle::default_spinner()
                    .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"])
                    .template("{spinner:.green} {msg}")
                    .unwrap(),
            );

            // Update the spinner message and tick
            progress_bar
                .as_ref()
                .unwrap()
                .enable_steady_tick(Duration::from_millis(100)); // update every 100ms
        }
    } else {
        progress_bar = None;
    }

    // Atomic counter to keep track of the number of items processed
    let counter = Arc::new(AtomicUsize::new(0));
    let done = Arc::new(AtomicBool::new(false));

    let handle: tokio::task::JoinHandle<()>;
    {
        // Start time to calculate elapsed time
        let start_time = Instant::now();
        let counter_clone = Arc::clone(&counter);
        let done_clone = done.clone();

        // thread to update the spinner message with items per second
        let pb = Arc::new(progress_bar.clone());
        handle = tokio::spawn(async move {
            if let Some(ref pb) = *pb {
                let mut last_count: usize = 0;
                loop {
                    // Calculate elapsed time
                    let elapsed = start_time.elapsed().as_secs_f64();

                    // Get the current count
                    let count = counter_clone.load(Ordering::SeqCst);

                    // Calculate items per second
                    let items_per_second = if elapsed > 0.0 {
                        count as f64 / elapsed
                    } else {
                        0.0
                    };

                    // Update spinner message
                    let this_count = counter_clone.load(Ordering::SeqCst);
                    pb.inc((this_count - last_count) as u64);
                    last_count = this_count;
                    let message = format!("#{} {:.2} items/second", this_count, items_per_second);
                    pb.set_message(message);

                    // Break the loop if work is done
                    let is_done = done_clone.load(Ordering::SeqCst);
                    if is_done {
                        break;
                    }
                    // Sleep for a short duration before updating again
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                }
            }
        });
    }

    let tasks;
    {
        tasks = tstream
            .map(|item| {
                let f = f.clone();
                let c: C = context.clone();
                let counter_clone = counter.clone();
                task::spawn(async move {
                    f(item, c).await;
                    counter_clone.fetch_add(1, Ordering::SeqCst);
                })
            })
            .buffer_unordered(concurrency) // Adjust concurrency level as needed
            .collect::<Vec<_>>()
            .await;
    }

    // terminate update thread
    done.store(true, Ordering::SeqCst);
    if let Err(e) = handle.await {
        error!("Progress update thread failed to join: {:?}", e);
    }

    for task in tasks {
        if let Err(e) = task {
            error!("Task failed: {:?}", e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    #[tokio::test]
    async fn test_consume_concurrently() {
        let items = vec![1, 2, 3, 4, 5];
        let results = Arc::new(Mutex::new(Vec::new()));

        async fn process_item(item: i32, results: Arc<Mutex<Vec<i32>>>) {
            // debug messages to ensure things indeed run concurrently
            println!("(1) Starting to process {}", item);
            {
                // scope mutex to ensure we don't block other tasks while we sleep
                let mut results = results.lock().await;
                results.push(item);
            }
            println!("(2) Sleeping after {}", item);
            // Simulate some async work
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            println!("(3) Done with {}", item);
        }

        let start_time = Instant::now();
        consume_concurrently(items, process_item, &results, false, Some(5)).await;
        let end_time = Instant::now();

        let elapsed_time = end_time - start_time;
        let elapsed_time_ms = elapsed_time.as_millis();

        // The code above should process and wait in parallel
        assert!(
            elapsed_time < Duration::from_millis(200),
            "Time taken: {}",
            elapsed_time_ms
        );

        let results = results.lock().await;
        assert_eq!(results.len(), 5);
        assert!(results.contains(&1));
        assert!(results.contains(&2));
        assert!(results.contains(&3));
        assert!(results.contains(&4));
        assert!(results.contains(&5));
    }
}
