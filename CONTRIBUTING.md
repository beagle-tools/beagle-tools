# Contributing

1. Read [`AGENTS.md`](AGENTS.md) (repo map and shared rules) and the
   `AGENTS.md` of the app you're changing.
2. Follow [`docs/CODING_STANDARDS.md`](docs/CODING_STANDARDS.md). It is the
   same for every app.
3. Run `make check-<app>` and `make standards` from the repo root before you
   open a PR. They run exactly what CI runs.
4. CI's `gate` job is the only required check. If it's green, the PR can be
   merged. There are no optional checks.

Branches, commit messages and PR descriptions are covered in section 4 of
the standards.
