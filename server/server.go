package main

import (
	"context"
	"fmt"
	"log"
	"sync"
	"time"
)

// Server manages signaling sessions between peers.
type Server struct {
	sessions     map[string]*Session
	mu           sync.RWMutex
	maxSessions  int
	sessionTTL   time.Duration
	relayLimiter *RateLimiter
}

// NewServer creates a Server with the given capacity, TTL, and relay rate limit.
func NewServer(maxSessions int, sessionTTL time.Duration, relayRateLimit int64) *Server {
	return &Server{
		sessions:     make(map[string]*Session),
		maxSessions:  maxSessions,
		sessionTTL:   sessionTTL,
		relayLimiter: NewRateLimiter(relayRateLimit),
	}
}

// GetOrCreateSession returns the session for code, creating one if needed.
// Returns an error if the requested role is already taken.
func (s *Server) GetOrCreateSession(code string, role string) (*Session, error) {
	s.mu.Lock()
	defer s.mu.Unlock()

	sess, exists := s.sessions[code]
	if exists {
		sess.mu.Lock()
		defer sess.mu.Unlock()

		if role == "sender" && sess.Sender != nil {
			return nil, fmt.Errorf("sender already connected for code %s", code)
		}
		if role == "receiver" && sess.Receiver != nil {
			return nil, fmt.Errorf("receiver already connected for code %s", code)
		}
		return sess, nil
	}

	// Check capacity before creating a new session.
	if len(s.sessions) >= s.maxSessions {
		return nil, fmt.Errorf("max sessions reached (%d)", s.maxSessions)
	}

	now := time.Now()
	sess = &Session{
		Code:      code,
		CreatedAt: now,
		ExpiresAt: now.Add(s.sessionTTL),
		relayDone: make(chan struct{}),
	}
	s.sessions[code] = sess
	return sess, nil
}

// RegisterPeer atomically reserves a role and attaches its peer to a session.
func (s *Server) RegisterPeer(code string, peer *Peer) (*Session, error) {
	s.mu.Lock()
	defer s.mu.Unlock()

	sess := s.sessions[code]
	if sess == nil {
		if len(s.sessions) >= s.maxSessions {
			return nil, fmt.Errorf("max sessions reached (%d)", s.maxSessions)
		}
		now := time.Now()
		sess = &Session{Code: code, CreatedAt: now, ExpiresAt: now.Add(s.sessionTTL), relayDone: make(chan struct{})}
		s.sessions[code] = sess
	}

	sess.mu.Lock()
	defer sess.mu.Unlock()
	switch peer.Role {
	case "sender":
		if sess.Sender != nil || sess.senderReserved {
			return nil, fmt.Errorf("sender already connected for code %s", code)
		}
		sess.senderReserved = true
		sess.Sender = peer
	case "receiver":
		if sess.Receiver != nil || sess.receiverReserved {
			return nil, fmt.Errorf("receiver already connected for code %s", code)
		}
		sess.receiverReserved = true
		sess.Receiver = peer
	default:
		return nil, fmt.Errorf("invalid role %q", peer.Role)
	}
	return sess, nil
}

// RemoveSession deletes a session only if it is still the mapped identity.
func (s *Server) RemoveSession(code string, expected *Session) bool {
	s.mu.Lock()
	defer s.mu.Unlock()
	if s.sessions[code] != expected {
		return false
	}
	delete(s.sessions, code)
	return true
}

// SessionCount returns the number of active sessions.
func (s *Server) SessionCount() int {
	s.mu.RLock()
	defer s.mu.RUnlock()
	return len(s.sessions)
}

// CloseAllSessions closes peer connections and releases handlers during shutdown.
func (s *Server) CloseAllSessions() {
	s.mu.RLock()
	sessions := make([]*Session, 0, len(s.sessions))
	for _, sess := range s.sessions {
		sessions = append(sessions, sess)
	}
	s.mu.RUnlock()

	for _, sess := range sessions {
		sess.Close()
		sess.signalRelayDone()
		s.RemoveSession(sess.Code, sess)
	}
}

// CleanupLoop periodically removes expired sessions.
func (s *Server) CleanupLoop(interval time.Duration) {
	s.CleanupLoopContext(nil, interval)
}

func (s *Server) CleanupLoopContext(ctx context.Context, interval time.Duration) {
	ticker := time.NewTicker(interval)
	defer ticker.Stop()
	var done <-chan struct{}
	if ctx != nil {
		done = ctx.Done()
	}

	for {
		select {
		case <-ticker.C:
			s.cleanupExpired()
		case <-done:
			return
		}
	}
}

func (s *Server) cleanupExpired() {
	now := time.Now()
	s.mu.Lock()
	defer s.mu.Unlock()

	for code, sess := range s.sessions {
		if now.After(sess.ExpiresAt) {
			log.Printf("cleaning up expired session %s", code)
			sess.Close()
			delete(s.sessions, code)
		}
	}
}
