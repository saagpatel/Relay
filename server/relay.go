package main

import (
	"io"
	"log"
	"sync"
	"time"

	"github.com/gorilla/websocket"
)

// RateLimiter implements a token bucket for relay bandwidth control.
type RateLimiter struct {
	mu         sync.Mutex
	tokens     float64
	maxTokens  float64
	refillRate float64 // bytes per second
	lastRefill time.Time
}

// NewRateLimiter creates a rate limiter with the given bytes-per-second limit.
func NewRateLimiter(bytesPerSecond int64) *RateLimiter {
	if bytesPerSecond < 1 {
		bytesPerSecond = 1
	}
	// Bucket holds 2 seconds worth of tokens for burst handling
	maxTokens := float64(bytesPerSecond) * 2.0
	return &RateLimiter{
		tokens:     maxTokens,
		maxTokens:  maxTokens,
		refillRate: float64(bytesPerSecond),
		lastRefill: time.Now(),
	}
}

// Wait blocks until n bytes worth of tokens are available, then consumes them.
func (r *RateLimiter) Wait(n int) {
	if n <= 0 {
		return
	}
	r.mu.Lock()
	defer r.mu.Unlock()

	remaining := float64(n)
	for remaining > 0 {
		r.refill()
		// A single frame can exceed the bucket. Consume it in bucket-sized
		// portions so it proceeds at the configured rate instead of waiting
		// forever for an impossible token count.
		needed := remaining
		if needed > r.maxTokens {
			needed = r.maxTokens
		}
		for r.tokens < needed {
			deficit := needed - r.tokens
			waitDuration := time.Duration(deficit / r.refillRate * float64(time.Second))
			if waitDuration < time.Millisecond {
				waitDuration = time.Millisecond
			}
			r.mu.Unlock()
			time.Sleep(waitDuration)
			r.mu.Lock()
			r.refill()
		}
		r.tokens -= needed
		remaining -= needed
	}
}

func (r *RateLimiter) refill() {
	now := time.Now()
	elapsed := now.Sub(r.lastRefill).Seconds()
	r.lastRefill = now

	r.tokens += elapsed * r.refillRate
	if r.tokens > r.maxTokens {
		r.tokens = r.maxTokens
	}
}

// relayLoop runs bidirectional WebSocket forwarding between sender and receiver.
// Both connections are switched to binary mode — all messages are forwarded as-is.
func relayLoop(sender *Peer, receiver *Peer, limiter *RateLimiter) {
	log.Printf("relay: starting bidirectional relay")

	var wg sync.WaitGroup
	finished := make(chan struct{}, 2)
	wg.Add(2)

	// sender → receiver
	go func() {
		defer wg.Done()
		forwardBinary("sender→receiver", sender.Conn, receiver, limiter)
		finished <- struct{}{}
	}()

	// receiver → sender
	go func() {
		defer wg.Done()
		forwardBinary("receiver→sender", receiver.Conn, sender, limiter)
		finished <- struct{}{}
	}()

	// Either direction ending terminates the whole relay and unblocks the
	// opposite ReadMessage call.
	<-finished
	sender.Close()
	receiver.Close()
	wg.Wait()
	log.Printf("relay: relay loop finished")
}

// forwardBinary reads binary WebSocket messages from src and writes them to dst.
func forwardBinary(label string, src *websocket.Conn, dst *Peer, limiter *RateLimiter) {
	for {
		messageType, data, err := src.ReadMessage()
		if err != nil {
			if websocket.IsUnexpectedCloseError(err, websocket.CloseNormalClosure, websocket.CloseGoingAway) {
				log.Printf("relay %s: read error: %v", label, err)
			}
			// Close the other side when one side disconnects
			dst.writeMu.Lock()
			_ = dst.Conn.SetWriteDeadline(time.Now().Add(writeTimeout))
			dst.Conn.WriteMessage(websocket.CloseMessage,
				websocket.FormatCloseMessage(websocket.CloseNormalClosure, ""))
			dst.writeMu.Unlock()
			return
		}

		// Only forward binary messages and apply rate limiting
		if messageType == websocket.BinaryMessage {
			limiter.Wait(len(data))

			dst.writeMu.Lock()
			_ = dst.Conn.SetWriteDeadline(time.Now().Add(writeTimeout))
			err = dst.Conn.WriteMessage(websocket.BinaryMessage, data)
			dst.writeMu.Unlock()

			if err != nil {
				if err != io.EOF {
					log.Printf("relay %s: write error: %v", label, err)
				}
				return
			}
		} else if messageType == websocket.CloseMessage {
			dst.writeMu.Lock()
			_ = dst.Conn.SetWriteDeadline(time.Now().Add(writeTimeout))
			dst.Conn.WriteMessage(websocket.CloseMessage, data)
			dst.writeMu.Unlock()
			return
		}
		// Ignore text/ping/pong in relay mode
	}
}
