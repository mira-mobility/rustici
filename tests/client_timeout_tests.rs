//! Integration tests for timeout functionality in the VICI client
//!
//! Place this file in: tests/client_timeout_tests.rs
//! Run with: cargo test

use rustici::{error::Error, Client};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

// Mock server implementation for testing
mod mock_vici_server {
    use std::io::{Read, Write};
    use std::os::unix::net::{UnixListener, UnixStream};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::thread;
    use std::time::{Duration, Instant};

    pub struct MockViciServer {
        listener: UnixListener,
        running: Arc<AtomicBool>,
        socket_path: String,
    }

    impl MockViciServer {
        pub fn new(socket_path: &str) -> std::io::Result<Self> {
            // Remove existing socket if it exists
            let _ = std::fs::remove_file(socket_path);

            let listener = UnixListener::bind(socket_path)?;
            listener.set_nonblocking(true)?;

            Ok(Self {
                listener,
                running: Arc::new(AtomicBool::new(true)),
                socket_path: socket_path.to_string(),
            })
        }

        /// Start server that sends events at specified intervals
        pub fn start_with_events(self, event_interval: Option<Duration>) -> MockServerHandle {
            let running = self.running.clone();
            let socket_path = self.socket_path.clone();

            let handle = thread::spawn(move || {
                while running.load(Ordering::Relaxed) {
                    match self.listener.accept() {
                        Ok((mut stream, _)) => {
                            let running = running.clone();
                            thread::spawn(move || {
                                handle_client(&mut stream, &running, event_interval);
                            });
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            thread::sleep(Duration::from_millis(10));
                        }
                        Err(_) => break,
                    }
                }
            });

            MockServerHandle {
                running: self.running,
                handle: Some(handle),
                socket_path,
            }
        }

        /// Start server that never sends events (for timeout testing)
        pub fn start_silent(self) -> MockServerHandle {
            self.start_with_events(None)
        }
    }

    fn handle_client(
        stream: &mut UnixStream,
        running: &Arc<AtomicBool>,
        event_interval: Option<Duration>,
    ) {
        let _ = stream.set_nonblocking(true);
        let mut last_event = Instant::now();

        while running.load(Ordering::Relaxed) {
            // Read any incoming packets (event registrations, etc.)
            let mut buf = [0u8; 1024];
            match stream.read(&mut buf) {
                Ok(0) => break, // Connection closed
                Ok(n) => {
                    // Parse packet to see if it's an event registration
                    if n >= 5 && buf[4] == 3 {
                        // EventRegister
                        // Send EventConfirm
                        let confirm = vec![0, 0, 0, 1, 5]; // Length=1, Type=EventConfirm
                        let _ = stream.write_all(&confirm);
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    // No data available
                }
                Err(_) => break,
            }

            // Send periodic events if configured
            if let Some(interval) = event_interval {
                if last_event.elapsed() >= interval {
                    send_test_event(stream);
                    last_event = Instant::now();
                }
            }

            thread::sleep(Duration::from_millis(10));
        }
    }

    fn send_test_event(stream: &mut UnixStream) {
        // Create a simple test event
        // PacketType::Event = 7
        let event_name = "test-event";
        let event_data = create_simple_message();

        let mut packet = vec![7]; // Event type
        packet.push(event_name.len() as u8);
        packet.extend_from_slice(event_name.as_bytes());
        packet.extend_from_slice(&event_data);

        let len = packet.len() as u32;
        let mut frame = len.to_be_bytes().to_vec();
        frame.extend_from_slice(&packet);

        let _ = stream.write_all(&frame);
    }

    fn create_simple_message() -> Vec<u8> {
        // Create a simple message with one key-value pair
        let mut msg = vec![];
        msg.push(3); // KEY_VALUE
        let key = "status";
        msg.push(key.len() as u8);
        msg.extend_from_slice(key.as_bytes());
        let value = "test";
        msg.extend_from_slice(&(value.len() as u16).to_be_bytes());
        msg.extend_from_slice(value.as_bytes());
        msg
    }

    pub struct MockServerHandle {
        running: Arc<AtomicBool>,
        #[allow(dead_code)]
        handle: Option<thread::JoinHandle<()>>,
        socket_path: String,
    }

    impl MockServerHandle {
        #[allow(dead_code)]
        pub fn stop(mut self) {
            self.running.store(false, Ordering::Relaxed);
            if let Some(handle) = self.handle.take() {
                let _ = handle.join();
            }
        }
    }

    impl Drop for MockServerHandle {
        fn drop(&mut self) {
            self.running.store(false, Ordering::Relaxed);
            let _ = std::fs::remove_file(&self.socket_path);
        }
    }
}

use mock_vici_server::MockViciServer;

/// Test that next_event_with_timeout times out when no events arrive
#[test]
fn test_next_event_with_timeout_actually_times_out() {
    let socket_path = "/tmp/test_vici_timeout.sock";

    // Start a mock server that never sends events
    let server = MockViciServer::new(socket_path).expect("Failed to create mock server");
    let _server_handle = server.start_silent();

    // Give server time to start
    thread::sleep(Duration::from_millis(100));

    // Connect client
    let mut client = Client::connect(socket_path).expect("Failed to connect");

    // Set a short read timeout
    client
        .set_read_timeout(Some(Duration::from_millis(100)))
        .expect("Failed to set timeout");

    // Register for events
    client
        .register_event("test-event")
        .expect("Failed to register event");

    // Try to get an event - should timeout
    let start = Instant::now();
    let result = client.next_event_with_timeout();
    let elapsed = start.elapsed();

    // Verify it timed out
    assert!(matches!(result, Err(Error::Timeout)));

    // Verify it actually waited approximately the timeout duration
    assert!(elapsed >= Duration::from_millis(90)); // Allow some margin
    assert!(elapsed < Duration::from_millis(200)); // But not too long
}

/// Test that try_next_event respects the specified timeout
#[test]
fn test_try_next_event_with_custom_timeout() {
    let socket_path = "/tmp/test_vici_try_timeout.sock";

    // Start a mock server that never sends events
    let server = MockViciServer::new(socket_path).expect("Failed to create mock server");
    let _server_handle = server.start_silent();

    // Give server time to start
    thread::sleep(Duration::from_millis(100));

    // Connect client
    let mut client = Client::connect(socket_path).expect("Failed to connect");

    // Register for events
    client
        .register_event("test-event")
        .expect("Failed to register event");

    // Test with different timeout values
    let timeouts = vec![
        Duration::from_millis(50),
        Duration::from_millis(100),
        Duration::from_millis(200),
    ];

    for timeout in timeouts {
        let start = Instant::now();
        let result = client.try_next_event(timeout);
        let elapsed = start.elapsed();

        // Should timeout
        assert!(matches!(result, Err(Error::Timeout)));

        // Should respect the timeout duration (with some margin for test stability)
        assert!(elapsed >= timeout.saturating_sub(Duration::from_millis(20)));
        assert!(elapsed < timeout + Duration::from_millis(50));
    }
}

/// Test that events are received when they arrive before timeout
#[test]
fn test_timeout_methods_receive_events_when_available() {
    let socket_path = "/tmp/test_vici_events_received.sock";

    // Start a mock server that sends events every 50ms
    let server = MockViciServer::new(socket_path).expect("Failed to create mock server");
    let _server_handle = server.start_with_events(Some(Duration::from_millis(50)));

    // Give server time to start
    thread::sleep(Duration::from_millis(100));

    // Connect client
    let mut client = Client::connect(socket_path).expect("Failed to connect");

    // Register for events
    client
        .register_event("test-event")
        .expect("Failed to register event");

    // Test with next_event_with_timeout
    client
        .set_read_timeout(Some(Duration::from_millis(200)))
        .expect("Failed to set timeout");

    let start = Instant::now();
    let result = client.next_event_with_timeout();
    let elapsed = start.elapsed();

    // Should receive event before timeout
    assert!(result.is_ok());
    let (event_name, _msg) = result.unwrap();
    assert_eq!(event_name, "test-event");
    assert!(elapsed < Duration::from_millis(100)); // Should be fast

    // Test with try_next_event
    let start = Instant::now();
    let result = client.try_next_event(Duration::from_millis(200));
    let elapsed = start.elapsed();

    // Should receive event before timeout
    assert!(result.is_ok());
    let (event_name, _msg) = result.unwrap();
    assert_eq!(event_name, "test-event");
    assert!(elapsed < Duration::from_millis(100)); // Should be fast
}

/// Test graceful shutdown pattern using timeout
#[test]
fn test_graceful_shutdown_with_timeout() {
    let socket_path = "/tmp/test_vici_shutdown.sock";

    // Start a mock server that rarely sends events
    let server = MockViciServer::new(socket_path).expect("Failed to create mock server");
    let _server_handle = server.start_with_events(Some(Duration::from_secs(10)));

    // Give server time to start
    thread::sleep(Duration::from_millis(100));

    // Connect client
    let client = Arc::new(Mutex::new(
        Client::connect(socket_path).expect("Failed to connect"),
    ));

    // Register for events
    client
        .lock()
        .unwrap()
        .register_event("test-event")
        .expect("Failed to register event");

    // Simulate event listener with graceful shutdown
    let running = Arc::new(AtomicBool::new(true));
    let running_clone = running.clone();
    let client_clone = client.clone();

    let listener_thread = thread::spawn(move || {
        let mut event_count = 0;

        while running_clone.load(Ordering::Relaxed) {
            // Use try_next_event with short timeout for responsive shutdown
            let result = client_clone
                .lock()
                .unwrap()
                .try_next_event(Duration::from_millis(100));

            match result {
                Ok((_event_name, _msg)) => {
                    event_count += 1;
                    // Process event...
                }
                Err(Error::Timeout) => {
                    // Timeout is expected and allows checking shutdown flag
                    continue;
                }
                Err(e) => {
                    eprintln!("Error receiving event: {:?}", e);
                    break;
                }
            }
        }

        event_count
    });

    // Let it run briefly
    thread::sleep(Duration::from_millis(300));

    // Signal shutdown
    let shutdown_start = Instant::now();
    running.store(false, Ordering::Relaxed);

    // Wait for thread to finish
    let event_count = listener_thread.join().expect("Failed to join thread");
    let shutdown_duration = shutdown_start.elapsed();

    // Verify graceful shutdown
    assert!(
        shutdown_duration < Duration::from_millis(200),
        "Shutdown took too long: {:?}",
        shutdown_duration
    );

    // Event count doesn't matter, just that we shut down gracefully
    println!("Processed {} events before shutdown", event_count);
}

/// Integration test simulating the actual use case from the issue
#[test]
fn test_event_listener_loop_with_stop_signal() {
    let socket_path = "/tmp/test_vici_integration.sock";

    // Start mock server
    let server = MockViciServer::new(socket_path).expect("Failed to create mock server");
    let _server_handle = server.start_with_events(Some(Duration::from_secs(10)));

    thread::sleep(Duration::from_millis(100));

    // This simulates the actual event listener pattern from the issue
    struct EventListener {
        running: Arc<AtomicBool>,
        client: Arc<Mutex<Option<Client>>>,
    }

    impl EventListener {
        fn new(socket_path: &str) -> Self {
            let mut client = Client::connect(socket_path).expect("Failed to connect");
            client
                .register_event("ike-updown")
                .expect("Failed to register");

            Self {
                running: Arc::new(AtomicBool::new(true)),
                client: Arc::new(Mutex::new(Some(client))),
            }
        }

        fn start(&self) -> thread::JoinHandle<()> {
            let running = self.running.clone();
            let client = self.client.clone();

            thread::spawn(move || {
                while running.load(Ordering::Relaxed) {
                    let mut client_guard = client.lock().unwrap();

                    if let Some(ref mut c) = *client_guard {
                        // THIS IS THE FIX: Use try_next_event instead of next_event
                        match c.try_next_event(Duration::from_millis(100)) {
                            Ok((event_name, message)) => {
                                println!("Event: {} - {:?}", event_name, message);
                            }
                            Err(Error::Timeout) => {
                                // Check running flag on timeout - allows graceful shutdown
                                continue;
                            }
                            Err(e) => {
                                println!("Error: {:?}", e);
                                // In real code, might try to reconnect here
                                thread::sleep(Duration::from_millis(500));
                            }
                        }
                    } else {
                        // Client disconnected, try to reconnect
                        thread::sleep(Duration::from_millis(500));
                    }
                }
            })
        }

        fn stop(&self) {
            self.running.store(false, Ordering::Relaxed);
        }
    }

    // Test the pattern
    let listener = EventListener::new(socket_path);
    let handle = listener.start();

    // Let it run
    thread::sleep(Duration::from_millis(300));

    // Stop it
    let stop_start = Instant::now();
    listener.stop();

    // Wait for thread
    handle.join().expect("Failed to join listener thread");
    let stop_duration = stop_start.elapsed();

    // Verify it stopped quickly (within timeout + small margin)
    assert!(
        stop_duration < Duration::from_millis(200),
        "Stop took too long: {:?}",
        stop_duration
    );
}
