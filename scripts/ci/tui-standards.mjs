// The TUI standards (docs/CODING_STANDARDS.md "Terminal UIs") as checks.
// standards/tui-standards.json is the bridge: beagle-tui's keymap tests pin
// the Rust data to it, and this pins the doc's table to it, so a change to
// any one of the three fails the gate. It also makes every crate that
// depends on beagle-tui either a registered TUI (with a keymap test that
// walks beagle_tui::keymap::STANDARD) or explicitly not a TUI.

/** The doc's keymap table rows, `[job, keys]`, in order. */
export function docKeymapRows(doc) {
  const start = doc.indexOf("| Job | Keys |");
  if (start < 0) return null;
  const rows = [];
  for (const line of doc.slice(start).split("\n").slice(2)) {
    if (!line.startsWith("|")) break;
    const cells = line.split("|").slice(1, -1).map((c) => c.trim());
    rows.push(cells);
  }
  return rows;
}

/**
 * Checks the TUI standards. `read(path)` returns a repo file's text (or
 * throws), `files` lists every repo file. Returns the violations.
 */
export function checkTuiStandards(read, files) {
  const errors = [];
  const data = JSON.parse(read("standards/tui-standards.json"));
  const doc = read("docs/CODING_STANDARDS.md");

  // 1. The doc's table is exactly the data's keymap.
  const rows = docKeymapRows(doc);
  if (!rows) {
    errors.push("docs/CODING_STANDARDS.md: the `| Job | Keys |` keymap table is missing");
  } else {
    const want = data.keymap.map((b) => [b.job, b.doc]);
    if (JSON.stringify(rows) !== JSON.stringify(want)) {
      errors.push(
        "docs/CODING_STANDARDS.md: the keymap table differs from standards/tui-standards.json " +
          "(and beagle_tui::keymap::STANDARD) — change all three together",
      );
    }
  }
  for (const { key, action } of data.github_writes) {
    if (!doc.includes(`\`${key}\` ${action}`)) {
      errors.push(`docs/CODING_STANDARDS.md: GitHub write \`${key}\` ${action} is not listed`);
    }
  }

  // 2. Every crate on beagle-tui is a registered TUI or says why not.
  const registered = new Set([...Object.keys(data.tuis), ...Object.keys(data.not_tuis)]);
  for (const f of files) {
    if (!f.endsWith("/Cargo.toml")) continue;
    const text = read(f);
    if (!text.includes("[package]") || !/^beagle-tui\b/m.test(text)) continue;
    const dir = f.slice(0, -"/Cargo.toml".length);
    if (!registered.has(dir)) {
      errors.push(
        `${f}: depends on beagle-tui but is not in standards/tui-standards.json ` +
          '("tuis" with its keymap test, or "not_tuis" with why)',
      );
    }
  }

  // 3. Every registered TUI's keymap test walks the shared data.
  for (const [dir, test] of Object.entries(data.tuis)) {
    let text;
    try {
      text = read(test);
    } catch {
      errors.push(`${dir}: keymap test ${test} is missing`);
      continue;
    }
    if (!text.includes("keymap::STANDARD") || !text.includes("follows_the_standard_keymap")) {
      errors.push(
        `${test}: needs a \`*_follows_the_standard_keymap\` test that walks beagle_tui::keymap::STANDARD`,
      );
    }
  }
  // 4. Every registered TUI draws its help sheet and footer with the shared
  //    beagle_tui::chrome pieces (one look in every tool).
  for (const [dir, testFile] of Object.entries(data.tuis)) {
    const sources = files
      .filter((f) => f.startsWith(`${dir}/src/`) && f.endsWith(".rs") && f !== testFile)
      .filter((f) => !f.includes("/tests/"))
      .map((f) => read(f));
    if (!sources.some((t) => t.includes("chrome::draw_help("))) {
      errors.push(`${dir}: draw the ? help with beagle_tui::chrome::draw_help`);
    }
    if (!sources.some((t) => t.includes("chrome::draw_footer(") || t.includes("chrome::footer_line("))) {
      errors.push(`${dir}: draw the footer with beagle_tui::chrome::draw_footer (or footer_line)`);
    }
  }
  return errors;
}
