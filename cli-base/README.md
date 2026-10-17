# beagle

One command for every Beagle tool. `beagle` installs each tool's CLI as an
extension, keeps them up to date (or pinned), and runs them:

```sh
beagle extensions available            # what you can install
beagle extensions install rca          # pick a version; Enter = latest
beagle rca …                           # run it
beagle rca update [--version 0.28.0]   # update or pin one extension
beagle extensions use rca 0.29.0       # switch back, offline
beagle update [--all]                  # update beagle (and every extension)
beagle doctor                          # check the install
```

Install: download `beagle-cli-<target>.tar.gz` from the
[`beagle-cli-latest`](https://github.com/beagle-tools/beagle-tools/releases/tag/beagle-cli-latest)
release and put `beagle` on your `PATH`.

Extensions live under `~/.local/share/beagle/extensions/`. Contributor notes:
[AGENTS.md](AGENTS.md).
