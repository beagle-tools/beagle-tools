// "find PR": pick any repo across your tracked orgs (searchable, shown
// as org/repo) and browse its open pull requests. Selecting a repo runs
// the same cache-first selectRepo() the rest of the app uses, so the
// list renders instantly and refreshes from GitHub behind a spinner.

import { Check, ChevronDown, GitPullRequest, Search } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import type { ReactElement, RefObject } from "react";

import { fuzzyScore } from "../lib/fuzzy";
import { parsePrUrl } from "../lib/prUrl";
import { sortPrs } from "../lib/sort";
import { useKeyNav } from "../state/keyNav";
import { useAppStore } from "../state/store";
import { InboxRow, rowKey } from "./PrRows";
import { QuickReviewControl } from "./QuickReviewControl";
import { Button, ProgressBar, Spinner } from "./ui";

const ALL = "*";

export function FindPrTab(): ReactElement {
  const orgRepos = useAppStore((s) => s.orgRepos);
  const selectedRepo = useAppStore((s) => s.selectedRepo);
  const selectRepo = useAppStore((s) => s.selectRepo);
  const prs = useAppStore((s) => s.prs);
  const prSort = useAppStore((s) => s.prSort);
  const prHasMore = useAppStore((s) => s.prHasMore);
  const loadMorePrs = useAppStore((s) => s.loadMorePrs);
  const syncing = useAppStore((s) => s.syncing);
  const allRepoProgress = useAppStore((s) => s.allRepoProgress);
  const openPr = useAppStore((s) => s.openPr);

  const sorted = useMemo(() => sortPrs(prs, prSort), [prs, prSort]);
  const loading = selectedRepo !== null && (syncing[`prs:${selectedRepo}`] ?? false);

  useEffect(() => {
    useKeyNav
      .getState()
      .setList(
        sorted.map((pr) => ({ slug: `${pr.repo.owner}/${pr.repo.name}`, number: pr.number })),
      );
  }, [sorted]);

  return (
    <div className="flex flex-col gap-3">
      <RepoPicker
        repos={orgRepos.map((r) => r.full_name)}
        selected={selectedRepo}
        onPick={(slug) => {
          void selectRepo(slug);
        }}
        onOpenPr={(slug, number) => {
          void openPr(slug, number);
        }}
      />

      {allRepoProgress ? (
        <ProgressBar
          done={allRepoProgress.done}
          total={allRepoProgress.total}
          label={`loading repos ${String(allRepoProgress.done)}/${String(allRepoProgress.total)}`}
        />
      ) : null}

      {selectedRepo === null ? (
        <div className="flex flex-col items-center gap-2 py-16 text-center text-sm text-muted">
          <GitPullRequest size={28} strokeWidth={1.5} />
          pick a repository to see its open pull requests
        </div>
      ) : sorted.length === 0 ? (
        <div className="px-3 py-10 text-center text-sm text-muted">
          {loading ? <Spinner label="loading pull requests…" /> : "no open pull requests"}
        </div>
      ) : (
        <div className="space-y-1.5">
          {loading ? <Spinner label="refreshing…" /> : null}
          {sorted.map((pr) => (
            <InboxRow key={rowKey(pr)} pr={pr} aside={<QuickReviewControl pr={pr} />} />
          ))}
          {prHasMore ? (
            <div className="flex justify-center pt-1">
              <Button
                onClick={() => {
                  void loadMorePrs();
                }}
                disabled={loading}
              >
                load more
              </Button>
            </div>
          ) : null}
        </div>
      )}
    </div>
  );
}

/** Repo slugs alphabetically, or ranked by fuzzy score against `query`
 * (non-matches dropped) when it isn't blank. */
function rankRepos(repos: string[], query: string): string[] {
  const sorted = [...repos].sort((a, b) => a.localeCompare(b));
  if (!query.trim()) return sorted;
  return sorted
    .map((slug) => ({ slug, rank: fuzzyScore(query, slug) }))
    .filter((x): x is { slug: string; rank: number } => x.rank !== null)
    .sort((a, b) => b.rank - a.rank)
    .map((x) => x.slug);
}

/** Closes a popover on a click outside `wrapRef` or on Escape, while open. */
function useDismiss(
  open: boolean,
  setOpen: (open: boolean) => void,
  wrapRef: RefObject<HTMLDivElement | null>,
): void {
  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => {
      if (wrapRef.current && !wrapRef.current.contains(e.target as Node)) setOpen(false);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setOpen(false);
    };
    document.addEventListener("mousedown", onDown);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDown);
      document.removeEventListener("keydown", onKey);
    };
  }, [open, setOpen, wrapRef]);
}

/** Searchable repo list. Collapsed it shows the current selection;
 * typing filters every tracked org's repos with the same fuzzy match
 * the command palette uses. */
function RepoPicker({
  repos,
  selected,
  onPick,
  onOpenPr,
}: {
  repos: string[];
  selected: string | null;
  onPick: (slug: string) => void;
  /** A pasted PR URL skips the repo entirely and opens that PR. */
  onOpenPr: (slug: string, number: number) => void;
}): ReactElement {
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const wrapRef = useRef<HTMLDivElement | null>(null);

  useDismiss(open, setOpen, wrapRef);

  const matches = useMemo(() => rankRepos(repos, query), [repos, query]);

  // Pasting a PR link is the fastest way in — don't make someone pick
  // the repo and then hunt for a number they already have.
  const pasted = parsePrUrl(query);
  const label = selected === ALL ? "all repositories" : (selected ?? "select a repository…");

  return (
    <div className="relative" ref={wrapRef}>
      <button
        type="button"
        onClick={() => {
          setOpen((o) => !o);
          setQuery("");
        }}
        className="flex h-9 w-full items-center gap-2 rounded-lg border border-edge bg-panel-2 px-3 text-sm text-cream transition-colors hover:border-sky"
      >
        <Search size={13} className="shrink-0 text-muted" />
        <span
          className={`min-w-0 flex-1 truncate text-left font-mono text-xs ${selected ? "" : "text-muted"}`}
        >
          {label}
        </span>
        <ChevronDown
          size={13}
          className={`shrink-0 text-muted transition-transform ${open ? "rotate-180" : ""}`}
        />
      </button>

      {open ? (
        <div className="animate-fade-up absolute inset-x-0 top-full z-40 mt-1 overflow-hidden rounded-lg border border-edge bg-panel shadow-2xl">
          <input
            autoFocus
            value={query}
            onChange={(e) => {
              setQuery(e.target.value);
            }}
            onKeyDown={(e) => {
              if (e.key === "Enter" && pasted) {
                onOpenPr(pasted.slug, pasted.number);
                setOpen(false);
              }
            }}
            placeholder="filter repos, or paste a PR URL…"
            className="w-full border-b border-edge bg-transparent px-3 py-2 text-xs text-cream outline-none placeholder:text-muted"
          />
          <div className="max-h-72 overflow-y-auto">
            {pasted ? (
              <button
                type="button"
                onClick={() => {
                  onOpenPr(pasted.slug, pasted.number);
                  setOpen(false);
                }}
                className="flex w-full items-center gap-2 border-b border-edge/60 bg-sky/10 px-3 py-2 text-left text-xs text-sky transition-colors hover:bg-sky/20"
              >
                <GitPullRequest size={12} className="shrink-0" />
                <span className="truncate font-mono">
                  open {pasted.slug}#{pasted.number}
                </span>
                <span className="ml-auto shrink-0 text-[10px] text-muted">↵</span>
              </button>
            ) : null}
            <PickerRow
              label="all repositories"
              active={selected === ALL}
              onPick={() => {
                onPick(ALL);
                setOpen(false);
              }}
            />
            {matches.map((slug) => (
              <PickerRow
                key={slug}
                label={slug}
                active={selected === slug}
                onPick={() => {
                  onPick(slug);
                  setOpen(false);
                }}
              />
            ))}
            {matches.length === 0 ? (
              <div className="px-3 py-4 text-center text-xs text-muted">
                no repos match — check your tracked orgs in settings
              </div>
            ) : null}
          </div>
        </div>
      ) : null}
    </div>
  );
}

function PickerRow({
  label,
  active,
  onPick,
}: {
  label: string;
  active: boolean;
  onPick: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onPick}
      className={`flex w-full items-center gap-2 px-3 py-1.5 text-left font-mono text-xs transition-colors hover:bg-panel-2 ${
        active ? "text-sky" : "text-cream"
      }`}
    >
      {active ? <Check size={11} className="shrink-0" /> : <span className="w-[11px] shrink-0" />}
      <span className="truncate">{label}</span>
    </button>
  );
}
