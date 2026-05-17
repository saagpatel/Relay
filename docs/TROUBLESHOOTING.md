# Troubleshooting Guide

## Common Issues

### Transfer Code Issues

#### "Invalid code" error

**Symptom**: Receiver enters code but gets "Invalid code" error

**Causes**:
- Code format incorrect (must be `digit-word-word`, e.g., `42-elephant-zebra`)
- Code expired (sessions expire after 10 minutes by default)
- Sender disconnected before receiver entered code

**Solutions**:
```bash
# Verify code format
echo "42-elephant-zebra" | grep -E '^\d{1,2}-[a-z]+-[a-z]+$'

# Check signaling server status
curl http://your-server:8080/health
```

#### "Code already in use" error

**Symptom**: Sender tries to use the same code twice

**Cause**: Previous session with same code still active

**Solution**:
- Wait for previous session to expire (10 min default)
- Restart sender app to generate new code
- Check server logs for session cleanup

### Connection Issues

#### "Connection timeout" during transfer

**Symptom**: Transfer starts but times out during negotiation

**Causes**:
- NAT/firewall blocks QUIC (UDP port)
- Signaling server unreachable
- Both peers behind restrictive NAT (relay should activate)

**Solutions**:

1. **Verify signaling server**:
   ```bash
   curl -v http://your-server:8080/health
   # Should return: {"status":"ok","active_sessions":N}
   ```

2. **Check firewall** (sender side):
   ```bash
   # Allow outbound UDP (QUIC)
   sudo ufw allow out proto udp to any port 1024:65535
   ```

3. **Verify relay fallback**:
   - Check connection status in UI (should show "Relay" badge)
   - If stuck on "Negotiating", restart both apps

4. **Test direct connectivity**:
   ```bash
   # From receiver machine, test sender's IP
   nc -zvu <sender-ip> <sender-port>
   ```

#### "Relay not available" error

**Symptom**: Direct QUIC fails and relay doesn't activate

**Causes**:
- Signaling server relay disabled
- Server at capacity (`--max-sessions` reached)
- Network issue between client and server

**Solutions**:
```bash
# Check server capacity
curl http://your-server:8080/health
# If active_sessions >= max-sessions, increase limit

# Restart signaling server with higher limits
./relay-server --max-sessions=5000 --relay-rate-limit=52428800
```

### Transfer Performance

#### Very slow transfer speed

**Symptom**: Transfer works but speed is <1 MB/s

**Causes**:
- Using relay instead of direct QUIC (relay is rate-limited to 10 MB/s default)
- Network bandwidth limitation
- Server relay rate limit too low

**Solutions**:

1. **Check connection type**:
   - Look for "Direct P2P" badge in UI (green) = good
   - "Relay" badge (yellow) = slower, but functional

2. **Improve direct connection**:
   - Use same WiFi network (sender + receiver on LAN)
   - Disable VPN temporarily
   - Check router firewall settings

3. **Increase relay rate limit** (if self-hosting):
   ```bash
   # 50 MB/s instead of default 10 MB/s
   ./relay-server --relay-rate-limit=52428800
   ```

#### Transfer stalls at 99%

**Symptom**: Progress bar reaches 99% but never completes

**Causes**:
- Checksum verification in progress (can take time for large files)
- Network packet loss causing retransmission
- File write permission issue (receiver side)

**Solutions**:

1. **Wait** — SHA-256 verification can take 10-30 seconds for GB files

2. **Check disk space** (receiver):
   ```bash
   df -h /path/to/download/folder
   ```

3. **Check logs** (receiver):
   - Look for "checksum mismatch" or "write error" messages

### File/Folder Issues

#### Files missing from transfer

**Symptom**: Sender selects 10 files, but receiver only gets 8

**Causes**:
- Hidden files excluded (by design)
- Permission errors on sender side
- Path too long (OS limitation)

**Solutions**:

1. **Check hidden files**:
   - Relay skips files starting with `.` (e.g., `.DS_Store`)
   - This is intentional to avoid system clutter

2. **Verify file permissions** (sender):
   ```bash
   ls -la /path/to/files
   # Ensure files are readable
   ```

3. **Check for errors**:
   - Look for "failed to read file" in logs

#### "Path traversal detected" error

**Symptom**: Folder transfer fails with path security error

**Cause**: File path contains `..` or absolute path

**Solution**:
- This is a security feature (prevents directory traversal attacks)
- Ensure folder structure doesn't use `..` in relative paths
- File a bug if legitimate structure is blocked

### Application Crashes

#### Tauri app won't start

**Symptom**: Double-click app, nothing happens

**Causes**:
- Missing system libraries (Linux)
- Corrupted app bundle (macOS)
- WebView2 missing (Windows)

**Solutions**:

**macOS**:
```bash
# Check if app is quarantined
xattr -d com.apple.quarantine Relay.app

# Run from terminal to see errors
open Relay.app
```

**Linux**:
```bash
# Install required dependencies
sudo apt-get install libwebkit2gtk-4.0-dev \
  libgtk-3-dev \
  libayatana-appindicator3-dev

# Run from terminal
./relay
```

#### App crashes during transfer

**Symptom**: App closes unexpectedly mid-transfer

**Causes**:
- Memory exhaustion (very large files)
- Backend panic (Rust crash)

**Solutions**:

1. **Check system resources**:
   ```bash
   # Monitor memory usage
   top -p $(pgrep relay)
   ```

2. **Check crash logs**:
   - **macOS**: `~/Library/Logs/Relay/`
   - **Linux**: `~/.local/share/Relay/logs/`

3. **File a bug report** with:
   - Crash log
   - Steps to reproduce
   - File size and count

### Server Issues

#### Server runs out of memory

**Symptom**: Server process killed by OOM (Out Of Memory)

**Cause**: Too many concurrent sessions or memory leak

**Solutions**:

1. **Check active sessions**:
   ```bash
   curl http://localhost:8080/health
   # If active_sessions is very high, reduce --max-sessions
   ```

2. **Monitor heap**:
   ```bash
   # Go profiler (if enabled)
   go tool pprof http://localhost:8080/debug/pprof/heap
   ```

3. **Restart server periodically**:
   - Use systemd with `Restart=always`
   - Monitor with cron job

#### Server logs "rate limit exceeded"

**Symptom**: Clients see slow transfer, server logs rate limit messages

**Cause**: Relay bandwidth limit reached (`--relay-rate-limit`)

**Solutions**:

1. **Increase limit**:
   ```bash
   # 50 MB/s = 52428800 bytes/sec
   ./relay-server --relay-rate-limit=52428800
   ```

2. **Encourage direct transfers**:
   - Users on same LAN should get automatic direct QUIC
   - Check firewall rules blocking QUIC

## Debugging

### Enable Debug Logging

**Server**:
```bash
export RUST_LOG=debug
./relay-server
```

**Client** (Tauri):
```bash
export RUST_LOG=debug
./relay
```

### Network Debugging

**Capture WebSocket traffic**:
```bash
# Use Wireshark with filter: websocket
wireshark -i any -f "tcp port 8080"
```

**Test signaling server manually**:
```bash
# WebSocket client test
websocat ws://localhost:8080/register
# Send: {"type":"register","code":"42-test-code","role":"sender"}
```

### File Integrity Check

**Verify SHA-256**:
```bash
# Sender side
sha256sum original-file.txt

# Receiver side
sha256sum received-file.txt

# Should match
```

## Getting Help

If issues persist:

1. **Search existing issues**: https://github.com/your-org/relay/issues
2. **Create new issue** with:
   - OS version (macOS/Linux/Windows)
   - Relay version (`relay --version`)
   - Steps to reproduce
   - Error messages or logs
3. **Community chat**: [Discord/Slack link]

## Known Limitations

- **Windows support**: Limited (Tauri WebView2 dependency)
- **Very large files** (>10 GB): May require more memory
- **Concurrent transfers**: One session per app instance
- **Pause/Resume**: Not currently supported (cancel + restart)
