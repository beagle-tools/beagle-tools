// Tree view of the PR's changed files. Clicking a file scrolls the
// differ to that file's card. Single-child folder chains are compressed
// ("src/lib" as one row), GitHub-style.

import {
  ChevronDown,
  ChevronRight,
  ChevronsLeft,
  ChevronsRight,
  FileCode2,
  Folder,
  FolderOpen,
  FolderTree,
} from "lucide-react";
import { useMemo, useState } from "react";
import type { ReactElement } from "react";

import { fileAnchorId } from "../lib/format";
import type { FileDiff } from "../lib/types";
import { useAppStore } from "../state/store";

interface TreeFolder {
  name: string;
  folders: TreeFolder[];
  files: { name: string; file: FileDiff }[];
}

function displayPath(file: FileDiff): string {
  return file.status === "removed" ? file.old_path : file.new_path;
}

function buildTree(diff: FileDiff[]): TreeFolder {
  const root: TreeFolder = { name: "", folders: [], files: [] };
  for (const file of diff) {
    const parts = displayPath(file).split("/");
    let node = root;
    for (const part of parts.slice(0, -1)) {
      let next = node.folders.find((f) => f.name === part);
      if (!next) {
        next = { name: part, folders: [], files: [] };
        node.folders.push(next);
      }
      node = next;
    }
    node.files.push({ name: parts[parts.length - 1] ?? "", file });
  }
  compress(root);
  sortTree(root);
  return root;
}

/** Merge single-child folders with no files into one row: a/b/c. */
function compress(node: TreeFolder) {
  for (const folder of node.folders) {
    while (folder.folders.length === 1 && folder.files.length === 0) {
      const only = folder.folders[0];
      if (!only) break;
      folder.name = `${folder.name}/${only.name}`;
      folder.files = only.files;
      folder.folders = only.folders;
    }
    compress(folder);
  }
}

function sortTree(node: TreeFolder) {
  node.folders.sort((a, b) => a.name.localeCompare(b.name));
  node.files.sort((a, b) => a.name.localeCompare(b.name));
  node.folders.forEach(sortTree);
}

function statusColor(file: FileDiff): string {
  switch (file.status) {
    case "added":
      return "bg-moss";
    case "removed":
      return "bg-ember";
    case "renamed":
      return "bg-amber";
    case "modified":
      return "bg-sky";
  }
}

const EMPTY_DIFF: FileDiff[] = [];

/// Collapsible tree column inside the PR view. Open/closed state is
/// remembered per PR (mount with key={prKey} so state re-reads on
/// PR switch); defaults to open.
export function FileTreePanel({ prKey }: { prKey: string }): ReactElement {
  const storageKey = `tandem-filetree-${prKey}`;
  const [open, setOpen] = useState(() => localStorage.getItem(storageKey) !== "closed");
  const fileCount = useAppStore((s) => s.bundle?.diff.length ?? 0);

  const toggle = () => {
    const next = !open;
    setOpen(next);
    localStorage.setItem(storageKey, next ? "open" : "closed");
  };

  if (!open) {
    return (
      <button
        type="button"
        onClick={toggle}
        title="show changed files"
        className="flex h-full w-8 shrink-0 flex-col items-center gap-2 border-r border-edge bg-panel/70 py-3 text-muted transition-colors hover:text-cream"
      >
        <FolderTree size={14} />
        <span className="text-[10px] font-semibold">{fileCount}</span>
        <ChevronsRight size={13} className="mt-auto" />
      </button>
    );
  }

  return (
    <aside className="flex h-full w-60 shrink-0 flex-col border-r border-edge bg-panel/70">
      <div className="flex items-center gap-1.5 border-b border-edge px-3 py-2 text-[11px] uppercase tracking-wide text-muted">
        <FolderTree size={12} />
        files ({fileCount})
        <button
          type="button"
          onClick={toggle}
          title="hide file tree"
          className="ml-auto inline-flex size-5 items-center justify-center rounded text-muted transition-colors hover:bg-panel-2 hover:text-cream"
        >
          <ChevronsLeft size={13} />
        </button>
      </div>
      <div className="min-h-0 flex-1 overflow-y-auto">
        <FileTree />
      </div>
    </aside>
  );
}

export function FileTree(): ReactElement {
  const diff = useAppStore((s) => s.bundle?.diff ?? EMPTY_DIFF);
  const root = useMemo(() => buildTree(diff), [diff]);

  if (diff.length === 0) {
    return (
      <div className="p-4 text-center text-xs text-muted">
        open a pull request to see its changed files
      </div>
    );
  }

  return (
    <div className="overflow-y-auto px-2 py-2">
      <FolderChildren node={root} depth={0} />
    </div>
  );
}

function FolderChildren({ node, depth }: { node: TreeFolder; depth: number }) {
  return (
    <>
      {node.folders.map((folder) => (
        <FolderRow key={folder.name} folder={folder} depth={depth} />
      ))}
      {node.files.map(({ name, file }) => (
        <FileRow key={displayPath(file)} name={name} file={file} depth={depth} />
      ))}
    </>
  );
}

function FolderRow({ folder, depth }: { folder: TreeFolder; depth: number }) {
  const [open, setOpen] = useState(true);
  return (
    <>
      <button
        type="button"
        onClick={() => {
          setOpen((o) => !o);
        }}
        style={{ paddingLeft: `${String(depth * 12 + 6)}px` }}
        className="flex w-full items-center gap-1.5 rounded-md py-1 text-left text-xs text-muted transition-colors hover:bg-panel-2 hover:text-cream"
      >
        {open ? <ChevronDown size={12} /> : <ChevronRight size={12} />}
        {open ? <FolderOpen size={13} /> : <Folder size={13} />}
        <span className="truncate font-mono text-[11px]">{folder.name}</span>
      </button>
      {open ? <FolderChildren node={folder} depth={depth + 1} /> : null}
    </>
  );
}

function FileRow({ name, file, depth }: { name: string; file: FileDiff; depth: number }) {
  const path = displayPath(file);
  return (
    <button
      type="button"
      title={path}
      onClick={() => {
        document
          .getElementById(fileAnchorId(path))
          ?.scrollIntoView({ behavior: "smooth", block: "start" });
      }}
      style={{ paddingLeft: `${String(depth * 12 + 24)}px` }}
      className="flex w-full items-center gap-1.5 rounded-md py-1 text-left text-xs transition-colors hover:bg-panel-2"
    >
      <span className={`size-1.5 shrink-0 rounded-full ${statusColor(file)}`} />
      <FileCode2 size={13} className="shrink-0 text-muted" />
      <span className="truncate font-mono text-[11px] text-cream">{name}</span>
      <span className="ml-auto shrink-0 pr-1 text-[10px]">
        <span className="text-moss">+{file.additions}</span>{" "}
        <span className="text-ember">−{file.deletions}</span>
      </span>
    </button>
  );
}
