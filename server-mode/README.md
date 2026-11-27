# server-mode

`beagle server-mode on` gets a Mac ready to serve the Beagle phone app in
one command: it runs your own setup steps (connect a VPN, say), starts
`beagle serve` and waits until it answers, checks what matters, keeps the
Mac awake while the screen stays locked, and ends with one line:
**Ready — open Beagle on your phone** (or what's wrong).

server-mode ships inside the one `beagle` binary; it has no release of its
own. macOS only.

## Use

```sh
beagle server-mode                          # on: until `beagle server-mode off`
beagle server-mode on -t 8                  # for 8 hours (decimals ok)
beagle server-mode on --start-up -f run.sh  # also run run.sh in the background
beagle server-mode on --on-stop stop.sh     # run stop.sh when server mode ends
beagle server-mode on --new-pairing         # rotate the pairing token, show the QR code
beagle server-mode on --no-serve            # don't start or stop beagle serve
beagle server-mode on --no-lock             # don't lock the screen now
beagle server-mode status                   # on/off, time left, the checklist
beagle server-mode log                      # the steps' and startup file's output
beagle server-mode lock                     # lock the screen now
beagle server-mode off                      # sleep again, stop beagle serve, run the after steps
```

`on` prints, for example:

```text
server mode on — awake for 4h
  ✓ step network
  ✓ beagle serve: answering at https://mac.example-tailnet.ts.net:8443
  ✓ certificate: 80 days left
  ✓ agentd: running
  ✓ exposure: nothing found answering beyond this Mac
  screen locked
Ready — open Beagle on your phone
```

## What `on` does, in order

1. **Your `[[before]]` steps** from `server-mode.toml`, each with `sh -c`.
   A step must exit 0; with `wait`, that command is retried until it exits
   0. A failing step stops `on` before anything starts (unless the step
   says `optional = true`, which makes it a warning).
2. **`beagle serve`** (unless `--no-serve` / `serve = false`): installs or
   starts its launchd agent `com.beagle.serve` and waits up to 30 seconds
   for `GET /v1/health` to answer. `--new-pairing` first rotates the
   token and shows the QR code (`beagle serve pair --rotate`).
3. **Awake**: `caffeinate -i -s` (no idle sleep, no system sleep on AC
   power; the display still sleeps). `-t <hours>` adds a timer that runs
   `beagle server-mode off` when it ends (`caffeinate -t` is the backstop).
   `-f <file>` runs the startup file in the background, in its own process
   group.
4. **Checks**: the server answers, its TLS certificate's days left (a
   warning under 14), agentd running (started if it is installed but down),
   and `beagle doctor`'s exposure warnings.
5. **Lock**: Ctrl-Cmd-Q through `osascript`. That needs Accessibility
   access for your terminal (System Settings → Privacy & Security →
   Accessibility); without it the display sleeps instead, which locks
   after the screen-lock delay.
6. **One summary**: "Ready — open Beagle on your phone", or "Not ready"
   with the ✗ lines above it.

`off`, or the timer, stops caffeinate, the timer and the startup file (and
everything it spawned), then stops `beagle serve` completely (`launchctl
bootout`, so launchd doesn't restart it; its token, phones and settings are
kept), then runs the `--on-stop` file and your `[[after]]` steps. If the
end came while the Mac slept (or it restarted), the next `status` finishes
the stop. On battery it still stays awake, and says to plug in. Closing the
lid still sleeps the Mac unless it is on power with an external display.

## Config: `~/.config/beagle/server-mode.toml`

Every key is optional; with no file, `on` is serve + awake + lock. Flags
override the file.

```toml
serve = true            # start beagle serve on `on`, stop it on `off`
new_pairing = false     # rotate the pairing token on every `on`
lock = true             # lock the screen once on
hours = 8               # default -t
start_up = "~/bin/run.sh"   # default -f (runs for the whole session)
on_stop = "~/bin/stop.sh"   # default --on-stop

# Run in order by `on`, before beagle serve starts. Each must exit 0.
[[before]]
name = "network"
run = "tailscale up"          # any command; this one connects Tailscale
wait = "tailscale ip -4"      # optional: retried until it exits 0
timeout = 60                  # seconds for `run`, and again for `wait`
optional = false              # true: a failure only warns

# Run by `off` / the timer, after beagle serve stops.
[[after]]
name = "note"
run = "echo server mode ended at $(date)"
```

The Tailscale lines are only an example: server-mode itself knows nothing
about any VPN. If `tailscale up` needs a login, the step fails with its
message; log in (menu-bar icon → Log in), then run `on` again. Leave
Tailscale connected after `off` simply by not adding an `[[after]]` step
for it.

Output of every step goes to the session log, `beagle server-mode log`.

## State

One value per file in `$XDG_STATE_HOME/beagle/server-mode/` (default
`~/.local/state/beagle/server-mode/`): `caffeinate.pid`, `timer.pid`,
`startup.pid`, `since`, `until`, `file`, `serve` (on started beagle serve),
`on_stop`, and `startup.log` (the session log). `off` removes all but the
log.

A session started by the older standalone `server-mode` script (state in
`~/.local/state/server-mode/`) is still seen by `status` and `log`, stopped
by `off`, and blocks `on` until it is off.
