package main

import (
	"context"
	"flag"
	"log"
	"net/http"
	"os"
	"os/signal"
	"syscall"
	"time"
)

func main() {
	cfg, err := loadConfigFromEnv()
	if err != nil {
		log.Fatalf("server configuration error: %v", err)
	}

	addr := flag.String("addr", cfg.Addr, "listen address")
	maxSessions := flag.Int("max-sessions", cfg.MaxSessions, "maximum concurrent sessions")
	sessionTTL := flag.Duration("session-ttl", cfg.SessionTTL, "session time-to-live")
	relayRateLimit := flag.Int64("relay-rate-limit", cfg.RelayRateLimit, "relay rate limit in bytes/sec (default 10 MB/s)")
	flag.Parse()
	if err := (serverConfig{Addr: *addr, MaxSessions: *maxSessions, SessionTTL: *sessionTTL, RelayRateLimit: *relayRateLimit}).validate(); err != nil {
		log.Fatalf("server configuration error: %v", err)
	}

	srv := NewServer(*maxSessions, *sessionTTL, *relayRateLimit)

	ctx, stop := signal.NotifyContext(context.Background(), os.Interrupt, syscall.SIGTERM)
	defer stop()
	go srv.CleanupLoopContext(ctx, 60*time.Second)

	mux := http.NewServeMux()
	mux.HandleFunc("GET /health", srv.HealthHandler)
	// Transitional compatibility path used by existing perf probes and legacy integrations.
	mux.HandleFunc("GET /api/health", srv.HealthHandler)
	mux.HandleFunc("GET /ws/{code}", srv.WebSocketHandler)

	log.Printf("Relay signaling server starting on %s (max-sessions=%d, session-ttl=%s, relay-rate-limit=%d B/s)",
		*addr, *maxSessions, *sessionTTL, *relayRateLimit)

	httpServer := &http.Server{Addr: *addr, Handler: mux, ReadHeaderTimeout: 10 * time.Second}
	go func() {
		<-ctx.Done()
		srv.CloseAllSessions()
		shutdownCtx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
		defer cancel()
		if err := httpServer.Shutdown(shutdownCtx); err != nil {
			log.Printf("server shutdown error: %v", err)
		}
	}()
	if err := httpServer.ListenAndServe(); err != nil && err != http.ErrServerClosed {
		log.Fatalf("server failed: %v", err)
	}
}
