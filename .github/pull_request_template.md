<!--
Title: Conventional Commits, scoped by app — feat(rca): …, fix(tandem): …, chore(utils): …
Labels: exactly one app:* and one release:* label. The release label is the
semver bump this merge cuts (release:major / release:minor, otherwise patch).
-->

## What

<!-- What changed, in a few bullets. -->

## Why

<!-- The problem this solves; link the issue. -->

## How tested

<!-- Commands run and their result, new/changed tests, manual flows exercised. -->

- [ ] `make check-<app>` passes
- [ ] `make standards` passes
- [ ] Every acceptance criterion in the issue is met (list evidence below)

## Release label

<!-- release:patch | release:minor | release:major — and one line why.
Merging releases beagle-<app>-vX.Y.Z (docs-only changes don't release). -->

## Notes for reviewers

<!-- Format/IPC migration notes, new runtime dependency justification
(docs/CODING_STANDARDS.md §2 Dependencies), follow-ups. "None" is fine. -->

Closes #
