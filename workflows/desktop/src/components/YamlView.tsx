import { useMemo, type ReactElement } from "react";

import { highlightYaml } from "../lib/yaml";

// A manifest, coloured: keys, strings, numbers, booleans, comments.
export function YamlView({ text }: { text: string }): ReactElement {
  const lines = useMemo(() => highlightYaml(text), [text]);
  return (
    <pre className="yaml">
      {lines.map((tokens, i) => (
        <div key={i}>
          {tokens.length === 0
            ? " "
            : tokens.map((t, j) => (
                <span key={j} className={`syn-${t.kind}`}>
                  {t.text}
                </span>
              ))}
        </div>
      ))}
    </pre>
  );
}
