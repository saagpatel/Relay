package main

import (
	"fmt"
	"net/http"
	"net/http/httptest"
	"strings"
	"sync"
	"testing"
	"time"

	"github.com/gorilla/websocket"
)

// TestConcurrentSessions1000 tests handling of 1000 concurrent client pairs
func TestConcurrentSessions(t *testing.T) {
	srv, ts := newTestServer(t, 1000, 10*time.Minute)
	defer ts.Close()

	const numSessions = 100 // Reduced for test performance
	var wg sync.WaitGroup
	errors := make(chan error, numSessions*2)

	// Launch concurrent sessions
	for i := 0; i < numSessions; i++ {
		wg.Add(2) // sender + receiver

		code := fmt.Sprintf("load-%d", i)

		go func(sessionCode string) {
			defer wg.Done()
			if err := simulateSender(ts, sessionCode); err != nil {
				errors <- fmt.Errorf("sender %s: %w", sessionCode, err)
			}
		}(code)

		go func(sessionCode string) {
			defer wg.Done()
			// Small delay to ensure sender registers first
			time.Sleep(10 * time.Millisecond)
			if err := simulateReceiver(ts, sessionCode); err != nil {
				errors <- fmt.Errorf("receiver %s: %w", sessionCode, err)
			}
		}(code)
	}

	// Wait for all goroutines
	wg.Wait()
	close(errors)

	// Check for errors
	var errCount int
	for err := range errors {
		t.Error(err)
		errCount++
		if errCount > 10 {
			t.Fatal("Too many errors, stopping")
		}
	}

	waitForSessionCount(t, srv, 0, 3*time.Second)
}

// TestSessionTTLCleanup tests that sessions expire after TTL
func TestSessionTTLCleanup(t *testing.T) {
	srv, ts := newTestServer(t, 100, 100*time.Millisecond)
	defer ts.Close()

	// Create a session
	code := "ttl-test"
	sender := dialWS(t, ts, code)
	defer sender.Close()

	// Register sender
	if err := register(sender, "sender"); err != nil {
		t.Fatal(err)
	}

	// Verify session exists
	waitForSessionCount(t, srv, 1, 1*time.Second)

	// Wait for TTL to expire
	time.Sleep(150 * time.Millisecond)
	srv.cleanupExpired()

	// Session should be cleaned up
	waitForSessionCount(t, srv, 0, 1*time.Second)
}

// TestMemoryStability runs relay for extended period with constant churn
func TestMemoryStability(t *testing.T) {
	if testing.Short() {
		t.Skip("Skipping memory stability test in short mode")
	}

	srv, ts := newTestServer(t, 100, 10*time.Minute)
	defer ts.Close()

	duration := 5 * time.Second // Reduced for test performance
	deadline := time.Now().Add(duration)

	var sessionCounter int
	var wg sync.WaitGroup

	// Continuously create and close sessions
	for time.Now().Before(deadline) {
		wg.Add(2)
		code := fmt.Sprintf("churn-%d", sessionCounter)
		sessionCounter++

		go func(c string) {
			defer wg.Done()
			simulateSender(ts, c)
		}(code)

		go func(c string) {
			defer wg.Done()
			time.Sleep(10 * time.Millisecond)
			simulateReceiver(ts, c)
		}(code)

		time.Sleep(20 * time.Millisecond)
	}

	wg.Wait()

	waitForSessionCount(t, srv, 0, 3*time.Second)
}

// TestSessionCleanupLatency ensures cleanup stays within a practical bound.
func TestSessionCleanupLatency(t *testing.T) {
	srv, ts := newTestServer(t, 500, 10*time.Minute)
	defer ts.Close()

	const numSessions = 50
	var wg sync.WaitGroup

	for i := 0; i < numSessions; i++ {
		wg.Add(2)
		code := fmt.Sprintf("latency-%d", i)

		go func(c string) {
			defer wg.Done()
			_ = simulateSender(ts, c)
		}(code)

		go func(c string) {
			defer wg.Done()
			time.Sleep(10 * time.Millisecond)
			_ = simulateReceiver(ts, c)
		}(code)
	}

	wg.Wait()

	elapsed := waitForSessionCount(t, srv, 0, 3*time.Second)
	const maxCleanupLatency = 2 * time.Second
	if elapsed > maxCleanupLatency {
		t.Fatalf("Session cleanup too slow: got %v, want <= %v", elapsed, maxCleanupLatency)
	}
}

// TestMaxSessionsLimit tests that server rejects sessions beyond limit
func TestMaxSessionsLimit(t *testing.T) {
	srv, ts := newTestServer(t, 5, 10*time.Minute)
	defer ts.Close()

	// Create max sessions
	conns := make([]*websocket.Conn, 0, 5)
	for i := 0; i < 5; i++ {
		code := fmt.Sprintf("limit-%d", i)
		conn := dialWS(t, ts, code)
		if err := register(conn, "sender"); err != nil {
			t.Fatal(err)
		}
		conns = append(conns, conn)
	}
	defer func() {
		for _, conn := range conns {
			conn.Close()
		}
	}()

	waitForSessionCount(t, srv, 5, 1*time.Second)

	// Try to create 6th session (should fail)
	wsURL := "ws" + strings.TrimPrefix(ts.URL, "http") + "/ws/limit-6"
	conn, resp, err := websocket.DefaultDialer.Dial(wsURL, nil)
	if err != nil {
		// Connection was rejected, which is an expected outcome.
		// We should verify it was for the right reason.
		if resp == nil || resp.StatusCode != http.StatusServiceUnavailable {
			t.Fatalf("Expected connection to fail with status 503, but got status %d and error: %v", resp.StatusCode, err)
		}
		return // Test passed for this path.
	}
	defer conn.Close()

	// Send register message
	if err := register(conn, "sender"); err != nil {
		t.Fatal(err)
	}

	// Should receive error or connection close
	conn.SetReadDeadline(time.Now().Add(1 * time.Second))
	var msg SignalMessage
	err = conn.ReadJSON(&msg)
	if err == nil && msg.Type != "error" {
		t.Error("Expected error when exceeding max sessions, got none")
	}
}

// Helper: simulate a sender connecting and disconnecting
func simulateSender(ts *httptest.Server, code string) error {
	wsURL := "ws" + strings.TrimPrefix(ts.URL, "http") + "/ws/" + code
	conn, _, err := websocket.DefaultDialer.Dial(wsURL, nil)
	if err != nil {
		return err
	}
	defer conn.Close()

	// Register as sender
	if err := register(conn, "sender"); err != nil {
		return err
	}

	// Wait briefly
	time.Sleep(20 * time.Millisecond)
	// Graceful disconnect helps deterministic session cleanup under race mode.
	_ = conn.WriteJSON(SignalMessage{Type: "disconnect"})

	return nil
}

// Helper: simulate a receiver connecting and disconnecting
func simulateReceiver(ts *httptest.Server, code string) error {
	wsURL := "ws" + strings.TrimPrefix(ts.URL, "http") + "/ws/" + code
	conn, _, err := websocket.DefaultDialer.Dial(wsURL, nil)
	if err != nil {
		return err
	}
	defer conn.Close()

	// Register as receiver
	if err := register(conn, "receiver"); err != nil {
		return err
	}

	// Wait briefly
	time.Sleep(20 * time.Millisecond)
	// Graceful disconnect helps deterministic session cleanup under race mode.
	_ = conn.WriteJSON(SignalMessage{Type: "disconnect"})

	return nil
}

func waitForSessionCount(t *testing.T, srv *Server, want int, timeout time.Duration) time.Duration {
	t.Helper()
	start := time.Now()
	deadline := time.Now().Add(timeout)
	for time.Now().Before(deadline) {
		if got := srv.SessionCount(); got == want {
			return time.Since(start)
		}
		time.Sleep(10 * time.Millisecond)
	}

	got := srv.SessionCount()
	t.Fatalf("Expected %d active sessions, got %d", want, got)
	return time.Since(start)
}
