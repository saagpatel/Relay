package main

import (
	"strings"
	"testing"
	"time"

	"github.com/gorilla/websocket"
)

// dialAllocate opens a sender connection on /ws, which asks the server to
// allocate a nameplate, and returns the nameplate it was given.
func dialAllocate(t *testing.T, wsBase string) (*websocket.Conn, string) {
	t.Helper()
	conn, _, err := websocket.DefaultDialer.Dial(wsBase+"/ws", nil)
	if err != nil {
		t.Fatalf("dial /ws: %v", err)
	}
	if err := register(conn, "sender"); err != nil {
		t.Fatalf("register: %v", err)
	}
	msg := readMsg(t, conn)
	if msg.Type != "allocated" || msg.Code == "" {
		t.Fatalf("expected allocated with a nameplate, got %+v", msg)
	}
	return conn, msg.Code
}

// The server hands out the smallest free nameplate, so the room it learns is
// a short number and never the words that form the SPAKE2 password.
func TestSenderGetsSmallestFreeNameplate(t *testing.T) {
	_, ts := newTestServer(t, 100, 10*time.Minute)
	defer ts.Close()
	base := "ws" + strings.TrimPrefix(ts.URL, "http")

	first, n1 := dialAllocate(t, base)
	defer first.Close()
	second, n2 := dialAllocate(t, base)
	if n1 != "1" || n2 != "2" {
		t.Fatalf("expected nameplates 1 and 2, got %q and %q", n1, n2)
	}

	// A receiver joins nameplate 1 and both get peer_joined.
	receiver := dialWS(t, ts, n1)
	defer receiver.Close()
	register(receiver, "receiver")
	if msg := readMsg(t, receiver); msg.Type != "peer_joined" {
		t.Fatalf("receiver expected peer_joined, got %q", msg.Type)
	}
	if msg := readMsg(t, first); msg.Type != "peer_joined" {
		t.Fatalf("sender expected peer_joined, got %q", msg.Type)
	}

	// Releasing nameplate 2 makes it the next one handed out.
	second.Close()
	deadline := time.Now().Add(2 * time.Second)
	for {
		third, n3 := dialAllocate(t, base)
		third.Close()
		if n3 == "2" {
			break
		}
		if time.Now().After(deadline) {
			t.Fatalf("nameplate 2 was not reused, got %q", n3)
		}
		time.Sleep(20 * time.Millisecond)
	}
}

// Only senders allocate; a receiver must name the room it is joining.
func TestReceiverCannotAllocate(t *testing.T) {
	_, ts := newTestServer(t, 100, 10*time.Minute)
	defer ts.Close()
	conn, _, err := websocket.DefaultDialer.Dial("ws"+strings.TrimPrefix(ts.URL, "http")+"/ws", nil)
	if err != nil {
		t.Fatalf("dial /ws: %v", err)
	}
	defer conn.Close()
	register(conn, "receiver")
	if msg := readMsg(t, conn); msg.Type != "error" {
		t.Fatalf("expected error, got %q", msg.Type)
	}
}

func TestAllocationRespectsMaxSessions(t *testing.T) {
	_, ts := newTestServer(t, 1, 10*time.Minute)
	defer ts.Close()
	base := "ws" + strings.TrimPrefix(ts.URL, "http")
	first, _ := dialAllocate(t, base)
	defer first.Close()
	conn, _, err := websocket.DefaultDialer.Dial(base+"/ws", nil)
	if err != nil {
		t.Fatalf("dial: %v", err)
	}
	defer conn.Close()
	register(conn, "sender")
	if msg := readMsg(t, conn); msg.Type != "error" {
		t.Fatalf("expected error at capacity, got %q", msg.Type)
	}
}
