// Markdown renderer for comment bodies (GitHub-flavored: tables, code
// fences, task lists). Inline HTML (e.g. <sub> from bots) renders via
// rehype-raw and is sanitized; HTML comments are stripped; @mentions of
// the signed-in user get a highlight mark. Styling in styles.css.

import type { Element, ElementContent, Root } from "hast";
import type { ComponentProps, ReactElement, ReactNode } from "react";
import ReactMarkdown from "react-markdown";
import rehypeRaw from "rehype-raw";
import rehypeSanitize, { defaultSchema } from "rehype-sanitize";
import remarkGfm from "remark-gfm";

import { followLink, resolveLink } from "../lib/codeAnchor";
import { useAppStore } from "../state/store";

// Sanitize strips unknown URL schemes, so `tandem:` has to be allowed
// explicitly for agent summaries to link into the diff.
const SCHEMA = {
  ...defaultSchema,
  protocols: {
    ...defaultSchema.protocols,
    href: [...(defaultSchema.protocols?.href ?? []), "tandem"],
  },
};

/** Every link in agent prose goes through here. A plain <a> would
 * navigate the webview and unload the app, so nothing is ever followed
 * natively: repo paths jump into the diff (or GitHub when the file
 * isn't part of this PR), and web links open in the browser. */
function MarkdownLink({ href, children }: { href?: string | undefined; children?: ReactNode }) {
  const pr = useAppStore((s) => s.bundle?.detail.pull_request ?? null);
  const target = resolveLink(href);
  if (target.kind === "none") return <span>{children}</span>;

  const title =
    target.kind === "external"
      ? target.url
      : `${target.anchor.path}${target.anchor.line === null ? "" : `:${String(target.anchor.line)}`} — in the diff, or on ${pr?.base_ref ?? "the base branch"}`;

  return (
    <a
      href={href}
      title={title}
      onClick={(e) => {
        e.preventDefault();
        followLink(target, pr?.repo ?? null, pr?.base_ref ?? "HEAD");
      }}
      className="cursor-pointer text-sky underline underline-offset-2 hover:text-cream"
    >
      {children}
    </a>
  );
}

/** Wrap `@login` occurrences in text nodes with <mark class="mention-you">.
 * Runs after sanitize so the mark survives; code/pre are left alone. */
function rehypeHighlightMention(login: string) {
  const escaped = login.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const pattern = new RegExp(`@${escaped}(?![\\w-])`, "gi");

  const split = (value: string): ElementContent[] | null => {
    const parts: ElementContent[] = [];
    let last = 0;
    for (const match of value.matchAll(pattern)) {
      const index = match.index;
      if (index > last) parts.push({ type: "text", value: value.slice(last, index) });
      parts.push({
        type: "element",
        tagName: "mark",
        properties: { className: ["mention-you"] },
        children: [{ type: "text", value: match[0] }],
      });
      last = index + match[0].length;
    }
    if (parts.length === 0) return null;
    if (last < value.length) parts.push({ type: "text", value: value.slice(last) });
    return parts;
  };

  const walk = (node: Root | Element) => {
    if (node.type === "element" && (node.tagName === "code" || node.tagName === "pre")) return;
    node.children = node.children.flatMap((child): ElementContent[] => {
      if (child.type === "text") {
        return split(child.value) ?? [child];
      }
      if (child.type === "element") walk(child);
      return [child as ElementContent];
    });
  };

  return () => (tree: Root) => {
    walk(tree);
  };
}

export function MarkdownBody({ text }: { text: string }): ReactElement {
  const viewer = useAppStore((s) => s.viewer);
  const cleaned = text.replace(/<!--[\s\S]*?-->/g, "");
  const plugins: ComponentProps<typeof ReactMarkdown>["rehypePlugins"] = [
    rehypeRaw,
    [rehypeSanitize, SCHEMA],
    ...(viewer ? [rehypeHighlightMention(viewer)] : []),
  ];
  return (
    <div className="md-body">
      <ReactMarkdown
        remarkPlugins={[remarkGfm]}
        rehypePlugins={plugins}
        components={{ a: MarkdownLink }}
      >
        {cleaned}
      </ReactMarkdown>
    </div>
  );
}
