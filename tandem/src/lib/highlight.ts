// Lightweight per-line syntax highlighting for the differ, using
// highlight.js core with only the major languages registered. Token
// colors live in styles.css (.hljs-*) and follow the theme.

import hljs from "highlight.js/lib/core";
import bash from "highlight.js/lib/languages/bash";
import c from "highlight.js/lib/languages/c";
import cpp from "highlight.js/lib/languages/cpp";
import csharp from "highlight.js/lib/languages/csharp";
import css from "highlight.js/lib/languages/css";
import go from "highlight.js/lib/languages/go";
import java from "highlight.js/lib/languages/java";
import javascript from "highlight.js/lib/languages/javascript";
import json from "highlight.js/lib/languages/json";
import kotlin from "highlight.js/lib/languages/kotlin";
import markdown from "highlight.js/lib/languages/markdown";
import php from "highlight.js/lib/languages/php";
import python from "highlight.js/lib/languages/python";
import ruby from "highlight.js/lib/languages/ruby";
import rust from "highlight.js/lib/languages/rust";
import sql from "highlight.js/lib/languages/sql";
import swift from "highlight.js/lib/languages/swift";
import typescript from "highlight.js/lib/languages/typescript";
import xml from "highlight.js/lib/languages/xml";
import yaml from "highlight.js/lib/languages/yaml";

hljs.registerLanguage("bash", bash);
hljs.registerLanguage("c", c);
hljs.registerLanguage("cpp", cpp);
hljs.registerLanguage("csharp", csharp);
hljs.registerLanguage("css", css);
hljs.registerLanguage("go", go);
hljs.registerLanguage("java", java);
hljs.registerLanguage("javascript", javascript);
hljs.registerLanguage("json", json);
hljs.registerLanguage("kotlin", kotlin);
hljs.registerLanguage("markdown", markdown);
hljs.registerLanguage("php", php);
hljs.registerLanguage("python", python);
hljs.registerLanguage("ruby", ruby);
hljs.registerLanguage("rust", rust);
hljs.registerLanguage("sql", sql);
hljs.registerLanguage("swift", swift);
hljs.registerLanguage("typescript", typescript);
hljs.registerLanguage("xml", xml);
hljs.registerLanguage("yaml", yaml);

// highlight.js ships no HCL grammar — register a lightweight one.
hljs.registerLanguage("terraform", (h) => ({
  name: "Terraform",
  aliases: ["tf", "hcl"],
  keywords: {
    keyword:
      "resource variable module provider output data locals terraform " +
      "for in if else dynamic content lifecycle depends_on count for_each",
    literal: "true false null",
  },
  contains: [
    h.COMMENT(/#/, /$/),
    h.COMMENT(/\/\//, /$/),
    h.COMMENT(/\/\*/, /\*\//),
    h.QUOTE_STRING_MODE,
    h.C_NUMBER_MODE,
    { className: "variable", begin: /\b(var|local|each|module|self)\./ },
    { className: "attr", begin: /^\s*[a-zA-Z_][\w-]*(?=\s*=)/ },
  ],
}));

const EXT_TO_LANG: Record<string, string> = {
  sh: "bash",
  bash: "bash",
  zsh: "bash",
  c: "c",
  h: "c",
  cc: "cpp",
  cpp: "cpp",
  cxx: "cpp",
  hpp: "cpp",
  cs: "csharp",
  css: "css",
  scss: "css",
  go: "go",
  java: "java",
  js: "javascript",
  jsx: "javascript",
  mjs: "javascript",
  cjs: "javascript",
  json: "json",
  kt: "kotlin",
  kts: "kotlin",
  md: "markdown",
  php: "php",
  py: "python",
  rb: "ruby",
  rs: "rust",
  sql: "sql",
  swift: "swift",
  ts: "typescript",
  tsx: "typescript",
  html: "xml",
  xml: "xml",
  svg: "xml",
  yml: "yaml",
  yaml: "yaml",
  tf: "terraform",
  tfvars: "terraform",
  hcl: "terraform",
};

export function languageForPath(path: string): string | null {
  const ext = path.split(".").pop()?.toLowerCase() ?? "";
  return EXT_TO_LANG[ext] ?? null;
}

/**
 * Highlight one diff line. Returns HTML (hljs-escaped) or null when the
 * language is unknown — caller falls back to plain text. Per-line
 * highlighting can't track multi-line constructs, which is an accepted
 * tradeoff for diff rendering.
 */
export function highlightLine(content: string, language: string): string | null {
  try {
    return hljs.highlight(content, { language, ignoreIllegals: true }).value;
  } catch {
    return null;
  }
}
