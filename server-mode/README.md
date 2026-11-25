# server-mode

`beagle server-mode` keeps a Mac awake as a little server — so `beagle
serve`, agentd and anything else you leave running keep answering — while
the screen stays locked.

server-mode ships inside the one `beagle` binary; it has no release of its
own. macOS only.

## Use

```sh
beagle server-mode                          # on: stay awake until `beagle server-mode off`
beagle server-mode on -t 8                  # stay awake for 8 hours (decimals ok)
beagle server-mode on --start-up -f run.sh  # also run run.sh in the background
beagle server-mode on --no-lock             # stay awake without locking now
beagle server-mode status                   # on/off, time left, startup file
beagle server-mode log                      # the startup file's last 50 lines
beagle server-mode lock                     # lock the screen now
beagle server-mode off                      # let it sleep again, stop the file
```

## How it works

- **Awake** is `caffeinate -i -s`: no idle sleep, and no system sleep on AC
  power. The display still sleeps and the screen still locks. No `sudo`.
- **Lid:** closing the lid still sleeps the Mac unless it is on power with
  an external display.
- **Lock:** `on` locks the screen right away with Ctrl-Cmd-Q (the Apple menu → Lock
  Screen shortcut) through `osascript`. That needs Accessibility access for
  your terminal (System Settings → Privacy & Security → Accessibility);
  without it, server-mode sleeps the display instead, which locks after the
  screen-lock delay.
- **Timer:** `-t <hours>` starts a timer that runs `beagle server-mode off`
  when it ends; `caffeinate -t` is the backstop if the timer dies.
- **Startup file:** `-f <file>` runs it in the background in its own process
  group, output in `startup.log`. Executable files run as-is (their shebang
  wins), anything else with bash. `off`, or the timer, stops it and
  everything it spawned (TERM, then KILL after about a second).
- **On battery** it still stays awake, and says to plug in.

## State

One value per file in `$XDG_STATE_HOME/beagle/server-mode/` (default
`~/.local/state/beagle/server-mode/`): `caffeinate.pid`, `timer.pid`,
`startup.pid`, `since`, `until`, `file` and `startup.log`. `off` removes all
but the log.

A session started by the older standalone `server-mode` script (state in
`~/.local/state/server-mode/`) is still seen by `status` and `log`, stopped
by `off`, and blocks `on` until it is off.
