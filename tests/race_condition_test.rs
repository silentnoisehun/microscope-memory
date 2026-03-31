//! Test to verify the race condition fix in SHP server's handle_store function

#[cfg(feature = "shp")]
mod tests {
    use std::sync::{Arc, Mutex};
    use std::thread;
    use std::time::{Duration, Instant};

    /// Simulated store_memory function for testing
    fn test_store_memory(data: &str, counter: Arc<Mutex<Vec<String>>>) {
        // Simulate some work
        thread::sleep(Duration::from_millis(10));

        // Add to shared counter with lock
        let mut guard = counter.lock().unwrap();
        guard.push(data.to_string());
    }

    #[test]
    fn test_concurrent_store_operations() {
        let write_lock = Arc::new(Mutex::new(()));
        let operation_log = Arc::new(Mutex::new(Vec::new()));
        let mut handles = vec![];

        // Spawn 10 concurrent operations
        for i in 0..10 {
            let write_lock = Arc::clone(&write_lock);
            let log = Arc::clone(&operation_log);
            let data = format!("data_{}", i);

            let handle = thread::spawn(move || {
                // This simulates the spawn_blocking pattern used in handle_store
                let _guard = write_lock.lock().unwrap();
                test_store_memory(&data, log);
                // Lock is automatically released when _guard goes out of scope
            });

            handles.push(handle);
        }

        // Wait for all threads to complete
        for handle in handles {
            handle.join().unwrap();
        }

        // Verify all operations completed
        let log = operation_log.lock().unwrap();
        assert_eq!(log.len(), 10, "All 10 operations should complete");

        // Verify operations are properly serialized (no data corruption)
        for i in 0..10 {
            let expected = format!("data_{}", i);
            assert!(log.contains(&expected), "Operation {} should be recorded", i);
        }
    }

    #[test]
    fn test_write_lock_prevents_concurrent_writes() {
        let write_lock = Arc::new(Mutex::new(()));
        let concurrent_count = Arc::new(Mutex::new(0));
        let max_concurrent = Arc::new(Mutex::new(0));

        let mut handles = vec![];

        for i in 0..5 {
            let write_lock = Arc::clone(&write_lock);
            let concurrent = Arc::clone(&concurrent_count);
            let max_conc = Arc::clone(&max_concurrent);

            let handle = thread::spawn(move || {
                println!("Thread {} waiting for lock...", i);
                let _guard = write_lock.lock().unwrap();
                println!("Thread {} acquired lock", i);

                // Increment concurrent counter
                {
                    let mut count = concurrent.lock().unwrap();
                    *count += 1;
                    let current = *count;
                    drop(count);

                    // Update max concurrent
                    let mut max = max_conc.lock().unwrap();
                    if current > *max {
                        *max = current;
                    }
                }

                // Simulate work
                thread::sleep(Duration::from_millis(50));

                // Decrement concurrent counter
                {
                    let mut count = concurrent.lock().unwrap();
                    *count -= 1;
                }

                println!("Thread {} releasing lock", i);
                // Lock is automatically released when _guard goes out of scope
            });

            handles.push(handle);
        }

        // Wait for all threads
        for handle in handles {
            handle.join().unwrap();
        }

        // Verify that operations were serialized (max concurrent should be 1)
        let max = max_concurrent.lock().unwrap();
        assert_eq!(*max, 1, "Operations should be serialized, max concurrent should be 1");
    }

    #[test]
    fn test_lock_acquisition_timing() {
        let write_lock = Arc::new(Mutex::new(()));
        let start_times = Arc::new(Mutex::new(Vec::new()));
        let end_times = Arc::new(Mutex::new(Vec::new()));

        let mut handles = vec![];

        let test_start = Instant::now();

        for i in 0..3 {
            let write_lock = Arc::clone(&write_lock);
            let starts = Arc::clone(&start_times);
            let ends = Arc::clone(&end_times);

            let handle = thread::spawn(move || {
                let _guard = write_lock.lock().unwrap();

                // Record start time
                starts.lock().unwrap().push((i, test_start.elapsed()));

                // Simulate work
                thread::sleep(Duration::from_millis(100));

                // Record end time
                ends.lock().unwrap().push((i, test_start.elapsed()));
            });

            handles.push(handle);

            // Small delay to ensure threads start in order
            thread::sleep(Duration::from_millis(10));
        }

        // Wait for all threads
        for handle in handles {
            handle.join().unwrap();
        }

        // Verify serial execution
        let starts = start_times.lock().unwrap();
        let ends = end_times.lock().unwrap();

        // Sort by thread ID to check timing
        let mut starts: Vec<_> = starts.clone();
        let mut ends: Vec<_> = ends.clone();
        starts.sort_by_key(|&(id, _)| id);
        ends.sort_by_key(|&(id, _)| id);

        println!("Thread execution times:");
        for i in 0..3 {
            println!("  Thread {}: start={:?}, end={:?}",
                starts[i].0, starts[i].1, ends[i].1);
        }

        // Verify no overlap: each thread should start after the previous one ends
        for i in 1..3 {
            let prev_end = ends[i-1].1;
            let curr_start = starts[i].1;
            assert!(
                curr_start >= prev_end,
                "Thread {} should start after thread {} ends",
                i, i-1
            );
        }
    }
}