// A small YAML highlighter for agent manifests: keys, strings, numbers,
// booleans/null, comments, list dashes, and block scalars (`|`, `>`) whose
// indented body reads as one string. Not a parser — it colours lines; the
// file on disk is the source of truth. Mirrors agenttop's highlight.rs.

export type TokenKind = "key" | "string" | "number" | "literal" | "comment" | "punct" | "plain";

export interface Token {
  kind: TokenKind;
  text: string;
}

const LITERALS = new Set(["true", "false", "null", "~", "yes", "no", "on", "off"]);
const NUMBER = /^-?(\d+(\.\d+)?|\.\d+)([eE][-+]?\d+)?$/;
const KEY = /^((?:"[^"]*"|'[^']*'|[^\s:#][^:#]*?))(\s*)(:)(?=\s|$)/;

/** Where an unquoted ` #` comment starts, or -1. */
function commentAt(line: string): number {
  let quote: string | null = null;
  for (let i = 0; i < line.length; i++) {
    const c = line[i];
    if (quote) {
      if (c === quote) quote = null;
    } else if (c === '"' || c === "'") {
      quote = c;
    } else if (c === "#" && (i === 0 || /\s/.test(line.charAt(i - 1)))) {
      return i;
    }
  }
  return -1;
}

function value(text: string): Token[] {
  const lead = /^\s*/.exec(text)?.[0] ?? "";
  const body = text.slice(lead.length);
  const trail = /\s*$/.exec(body)?.[0] ?? "";
  const core = body.slice(0, body.length - trail.length);
  const out: Token[] = lead ? [{ kind: "plain", text: lead }] : [];
  if (!core) return out.concat(trail ? [{ kind: "plain", text: trail }] : []);
  // Unquoted scalars are strings too — in a manifest they are names,
  // paths and commands, and reading them as values is the point.
  let kind: TokenKind = "string";
  if (LITERALS.has(core.toLowerCase())) kind = "literal";
  else if (NUMBER.test(core)) kind = "number";
  else if (/^[|>][-+]?\d*$/.test(core) || /^[[\]{}]+$/.test(core)) kind = "punct";
  out.push({ kind, text: core });
  if (trail) out.push({ kind: "plain", text: trail });
  return out;
}

/** One line, outside a block scalar. */
function line(text: string): Token[] {
  const out: Token[] = [];
  const hash = commentAt(text);
  const code = hash >= 0 ? text.slice(0, hash) : text;
  const comment = hash >= 0 ? text.slice(hash) : "";

  let rest = code;
  const indent = /^\s*/.exec(rest)?.[0] ?? "";
  if (indent) out.push({ kind: "plain", text: indent });
  rest = rest.slice(indent.length);
  // List dashes, possibly nested on one line: "- - x".
  for (;;) {
    const dash = /^-(\s+|$)/.exec(rest);
    if (!dash) break;
    out.push({ kind: "punct", text: dash[0] });
    rest = rest.slice(dash[0].length);
  }
  const key = KEY.exec(rest);
  if (key) {
    out.push({ kind: "key", text: key[1] ?? "" });
    if (key[2]) out.push({ kind: "plain", text: key[2] });
    out.push({ kind: "punct", text: key[3] ?? ":" });
    rest = rest.slice(key[0].length);
  }
  out.push(...value(rest));
  if (comment) out.push({ kind: "comment", text: comment });
  return out;
}

/** Every line of `text`, tokenised. */
export function highlightYaml(text: string): Token[][] {
  const lines = text.split("\n");
  const out: Token[][] = [];
  // Inside a block scalar: lines indented past `blockIndent` are its body.
  let blockIndent: number | null = null;
  for (const raw of lines) {
    const indent = /^\s*/.exec(raw)?.[0].length ?? 0;
    if (blockIndent !== null) {
      if (raw.trim() === "" || indent > blockIndent) {
        out.push([{ kind: "string", text: raw }]);
        continue;
      }
      blockIndent = null;
    }
    const tokens = line(raw);
    out.push(tokens);
    const code = (commentAt(raw) >= 0 ? raw.slice(0, commentAt(raw)) : raw).trimEnd();
    if (/:\s*[|>][-+]?\d*$/.test(code)) blockIndent = indent;
  }
  return out;
}
