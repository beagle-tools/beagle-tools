# rca/web — agent notes

Read [`../AGENTS.md`](../AGENTS.md) (rca) and
[`../../docs/CODING_STANDARDS.md`](../../docs/CODING_STANDARDS.md) first.
This file only covers what is specific to the public postmortem site.

## What it is

A static Astro site built from the `rcas/` workspaces at build time
(`src/lib/rcas.ts`; `BEAGLE_RCAS_DIR` overrides the default `../rcas`).
There is no server and no runtime data fetching.

## Publish rules (the invariant that matters)

- Only workspaces whose `rca.toml` has `published = true` are rendered.
- Only client-safe sections are emitted: Summary, Timeline, Root Cause,
  Impact, Resolution, Diagrams (`PUBLIC_SECTIONS` in `src/lib/rcas.ts`).
  Notes, Log and Final Review are internal and must never reach `dist/`.
- Markdown (`src/lib/render.ts`, via marked) and ANSI diagrams
  (`src/lib/ansi.ts`, which HTML-escapes its text) are rendered at build
  time and injected with `set:html`. marked passes raw HTML in section
  markdown through, so section authors are trusted.

## Layout

```text
src/lib/        build-time reader + pure helpers (format, text, render, ansi)
                with vitest tests
src/pages/      index (history) + incidents/[slug] (one postmortem)
src/components/ presentational Astro components
src/layouts/    Base.astro — imports src/styles/* in cascade order
```

Astro types template elements as `any`, so `.map()` callbacks in templates
are annotated `: unknown` to satisfy `no-unsafe-return` without disabling it.

## Commands

```sh
corepack pnpm dev    # local preview
make check-rca       # from the repo root: the full rca gate
```

Deploy and image publishing (`ghcr.io/beagle-tools/beagle-rca-web`, cut with
every `beagle-rca-v*` release) are documented in
[`DEPLOY.md`](DEPLOY.md).
