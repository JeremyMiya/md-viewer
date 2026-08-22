# Feature: X11 fallback when the Wayland socket is dead (#65)

**Status:** ✅ Complete
**Branch:** `fix/65-winit-wayland-fallback`
**Date:** 2026-08-22
**Lines Changed:** +85 / -3 in `src/main.rs`

## Summary

Snap-installed md-viewer aborted at startup on Ubuntu 26.04 (Wayland) with
`Error: WinitEventLoop(Os(... WaylandError(Connection(NoCompositor))))`. The
session had `WAYLAND_DISPLAY` set but the socket could not be connected from
inside the snap's confinement, and winit never considered Xwayland. The fix
probes the Wayland socket before eframe starts and clears the Wayland
variables when it is unreachable, so winit selects X11 up front.

## Features

- [x] Pre-flight Unix-socket probe of `$XDG_RUNTIME_DIR/$WAYLAND_DISPLAY`
- [x] Clear `WAYLAND_DISPLAY`/`WAYLAND_SOCKET` on connect failure (forces winit's X11 path)
- [x] Trust inherited `WAYLAND_SOCKET` fds (cannot be probed without consuming them)
- [x] Unit tests for socket path resolution mirroring libwayland's lookup rules

## Key Discoveries

### winit 0.30 has no backend fallback — and no second chance

winit 0.30's Linux `EventLoop::new()` picks **exactly one** backend from the
environment: non-empty `WAYLAND_DISPLAY`/`WAYLAND_SOCKET` → Wayland, else
`DISPLAY` → X11, else error. There is no try-Wayland-catch-X11 chain, so a
set-but-unusable Wayland socket is fatal even on sessions with working
Xwayland.

```rust
// winit-0.30.12/src/platform_impl/linux/mod.rs
(None, true, _) => Backend::Wayland, // WAYLAND_DISPLAY set → committed
```

A post-failure retry inside the same process is impossible: creating a second
event loop returns `EventLoopError::RecreationAttempt`. The first attempt in
this branch did exactly that:

```text
WARN: Wayland event loop failed (... NoCompositor); retrying with X11
Error: WinitEventLoop(RecreationAttempt)
```

Hence the decision must happen **before** `eframe::run_native`.

### A std-only Wayland reachability probe

libwayland's lookup rules are simple enough to mirror without adding a
dependency: absolute `WAYLAND_DISPLAY` passes through, otherwise it joins
`XDG_RUNTIME_DIR`. A `UnixStream::connect` then tells us what libwayland will
find. Snap confinement failures (AppArmor denial, missing socket) surface as
connect errors exactly like a missing compositor, which is what makes this
work for the snap case.

### Verification setup

Reproduced #65 locally by pointing `WAYLAND_DISPLAY` at a nonexistent socket
on Xvfb :99: pre-fix the process died, post-fix it logs one warning and opens
via X11. The inverse case was checked with a dummy listening `AF_UNIX`
socket — the probe accepts it and leaves the variables untouched.

## Future Improvements

- Root-cause why Ubuntu 26.04 snaps cannot reach a socket that the session
  exposes (snapd interface vs. socket naming); the app-level fallback covers
  users either way.
- Consider the same guard for other eframe apps built from this template.
