# Contributing

Thanks for wanting to help the pack. Bug reports, docs fixes and code are
all welcome.

## Report a bug or ask for something

[Open an issue](https://github.com/beagle-tools/beagle-tools/issues/new/choose).
Say which tool, what you ran, what you expected and what happened instead.
Include `beagle version` and `beagle doctor` output for CLI problems.

## Send a change

1. Set up your machine with
   [`docs/GETTING_STARTED.md`](docs/GETTING_STARTED.md#path-b-hack-on-the-repo)
   (`make bootstrap`).
2. Read [`AGENTS.md`](AGENTS.md) for the repo map and shared rules, then the
   `AGENTS.md` of the app you're changing. They're written for coding agents,
   and they work just as well for people.
3. Follow [`docs/CODING_STANDARDS.md`](docs/CODING_STANDARDS.md). It is the
   same for every app. Section 4 covers branches, commit messages and PR
   descriptions.
4. Add the tests [`docs/TESTING.md`](docs/TESTING.md) asks for.
5. Run `make check-<app>` and `make standards` from the repo root. They run
   exactly what CI runs.
6. Open a PR using the template. Give it one `app:*` label, plus a
   `release:*` label if it changes an app's code
   ([`docs/RELEASING.md`](docs/RELEASING.md)).

CI's `gate` job is the only required check. If it's green, the PR can be
merged. There are no optional checks.

## Keep it public-safe

This is a public repo:

- No secrets or tokens, and no company, client or private repo names.
- Use neutral examples like `acme/widgets` and `ABC-123`.
- Commit with your GitHub noreply address.
  [`scripts/check-commit-email.sh`](scripts/check-commit-email.sh) checks it.

## License

By contributing, you agree your work is licensed under the repo's
[Apache-2.0 license](LICENSE).
