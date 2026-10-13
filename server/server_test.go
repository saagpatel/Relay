package main

import (
	"context"
	"encoding/json"
	"errors"
	"net"
	"net/http"
	"net/http/httptest"
	"testing"
	"time"
)

func TestHealthEndpoint(t *testing.T) {
	srv := NewServer(100, 10*time.Minute, 10*1024*1024)

	req := httptest.NewRequest(http.MethodGet, "/health", nil)
	w := httptest.NewRecorder()
	srv.HealthHandler(w, req)

	if w.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d", w.Code)
	}

	var resp HealthResponse
	if err := json.NewDecoder(w.Body).Decode(&resp); err != nil {
		t.Fatalf("failed to decode response: %v", err)
	}
	if resp.Status != "ok" {
		t.Errorf("expected status ok, got %s", resp.Status)
	}
	if resp.ActiveSessions != 0 {
		t.Errorf("expected 0 active sessions, got %d", resp.ActiveSessions)
	}
}

func TestRegisterPeerReservesRoleAtomically(t *testing.T) {
	srv := NewServer(10, time.Minute, 1000)
	start := make(chan struct{})
	results := make(chan error, 2)
	for i := 0; i < 2; i++ {
		go func() {
			<-start
			peer := &Peer{Role: "sender", Done: make(chan struct{})}
			_, err := srv.RegisterPeer("same-role", peer)
			results <- err
		}()
	}
	close(start)
	first, second := <-results, <-results
	if (first == nil) == (second == nil) {
		t.Fatalf("expected exactly one role reservation to succeed, got %v and %v", first, second)
	}
}

func TestRemoveSessionChecksIdentity(t *testing.T) {
	srv := NewServer(10, time.Minute, 1000)
	old, err := srv.GetOrCreateSession("reused", "sender")
	if err != nil {
		t.Fatal(err)
	}
	srv.mu.Lock()
	newSession := &Session{Code: "reused"}
	srv.sessions["reused"] = newSession
	srv.mu.Unlock()
	if srv.RemoveSession("reused", old) {
		t.Fatal("old session removed a replacement session")
	}
	if srv.SessionCount() != 1 {
		t.Fatalf("replacement session missing, count=%d", srv.SessionCount())
	}
}

func TestCleanupLoopStopsWithContext(t *testing.T) {
	srv := NewServer(10, time.Minute, 1000)
	ctx, cancel := context.WithCancel(context.Background())
	done := make(chan struct{})
	go func() {
		srv.CleanupLoopContext(ctx, time.Hour)
		close(done)
	}()
	cancel()
	select {
	case <-done:
	case <-time.After(time.Second):
		t.Fatal("cleanup loop did not stop after cancellation")
	}
}

func TestCloseAllSessionsReleasesConnectionsAndHandlers(t *testing.T) {
	srv := NewServer(10, time.Minute, 1000)
	sess, err := srv.GetOrCreateSession("shutdown", "sender")
	if err != nil {
		t.Fatal(err)
	}
	sess.relayDone = make(chan struct{})
	srv.CloseAllSessions()
	if srv.SessionCount() != 0 {
		t.Fatalf("sessions remain after shutdown: %d", srv.SessionCount())
	}
	select {
	case <-sess.relayDone:
	default:
		t.Fatal("shutdown did not release relay waiters")
	}
}

func TestGracefulHTTPServerShutdown(t *testing.T) {
	listener, err := net.Listen("tcp", "127.0.0.1:0")
	if err != nil {
		t.Fatal(err)
	}
	server := &http.Server{Handler: http.HandlerFunc(func(w http.ResponseWriter, _ *http.Request) { w.WriteHeader(http.StatusOK) })}
	serveDone := make(chan error, 1)
	go func() { serveDone <- server.Serve(listener) }()

	response, err := http.Get("http://" + listener.Addr().String())
	if err != nil {
		t.Fatal(err)
	}
	_ = response.Body.Close()
	if response.StatusCode != http.StatusOK {
		t.Fatalf("unexpected status before shutdown: %d", response.StatusCode)
	}

	ctx, cancel := context.WithTimeout(context.Background(), time.Second)
	defer cancel()
	if err := server.Shutdown(ctx); err != nil {
		t.Fatalf("graceful shutdown failed: %v", err)
	}
	if err := <-serveDone; !errors.Is(err, http.ErrServerClosed) {
		t.Fatalf("Serve returned %v, want http.ErrServerClosed", err)
	}
}

func TestHealthEndpointTransitionalCompatibility(t *testing.T) {
	srv := NewServer(100, 10*time.Minute, 10*1024*1024)
	mux := http.NewServeMux()
	mux.HandleFunc("GET /health", srv.HealthHandler)
	mux.HandleFunc("GET /api/health", srv.HealthHandler)
	ts := httptest.NewServer(mux)
	defer ts.Close()

	for _, route := range []string{"/health", "/api/health"} {
		req, err := http.NewRequest(http.MethodGet, ts.URL+route, nil)
		if err != nil {
			t.Fatalf("failed to create request for %s: %v", route, err)
		}
		res, err := http.DefaultClient.Do(req)
		if err != nil {
			t.Fatalf("health request failed for %s: %v", route, err)
		}
		if res.StatusCode != http.StatusOK {
			t.Fatalf("expected 200 from %s, got %d", route, res.StatusCode)
		}
		_ = res.Body.Close()
	}
}

func TestSessionCreation(t *testing.T) {
	srv := NewServer(100, 10*time.Minute, 10*1024*1024)

	sess, err := srv.GetOrCreateSession("abc123", "sender")
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if sess.Code != "abc123" {
		t.Errorf("expected code abc123, got %s", sess.Code)
	}
	if srv.SessionCount() != 1 {
		t.Errorf("expected 1 session, got %d", srv.SessionCount())
	}
}

func TestSessionExpiry(t *testing.T) {
	// Use a very short TTL so sessions expire immediately.
	srv := NewServer(100, 1*time.Millisecond, 10*1024*1024)

	_, err := srv.GetOrCreateSession("expire-me", "sender")
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	// Wait for the session to expire.
	time.Sleep(10 * time.Millisecond)
	srv.cleanupExpired()

	if srv.SessionCount() != 0 {
		t.Errorf("expected 0 sessions after cleanup, got %d", srv.SessionCount())
	}
}

func TestMaxSessions(t *testing.T) {
	srv := NewServer(2, 10*time.Minute, 10*1024*1024)

	_, err := srv.GetOrCreateSession("s1", "sender")
	if err != nil {
		t.Fatalf("unexpected error creating session 1: %v", err)
	}
	_, err = srv.GetOrCreateSession("s2", "sender")
	if err != nil {
		t.Fatalf("unexpected error creating session 2: %v", err)
	}

	// Third session should fail.
	_, err = srv.GetOrCreateSession("s3", "sender")
	if err == nil {
		t.Fatal("expected error when exceeding max sessions, got nil")
	}
}
