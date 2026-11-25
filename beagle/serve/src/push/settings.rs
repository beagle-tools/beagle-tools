//! Which events each phone wants: the catalog of tools and their event
//! kinds (from each tool's crate), and a phone's per-tool settings.

use std::collections::BTreeMap;

use beagle_scout::events::Kind;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// One tool's events, as the app lists them.
#[derive(Debug, Clone, Copy)]
pub struct ToolCatalog {
    /// The tool (`scout`).
    pub tool: &'static str,
    /// Its name in the app.
    pub name: &'static str,
    /// Its event kinds, from the tool's crate.
    pub kinds: &'static [Kind],
}

/// Every tool that sends events. A tool joins by writing its spool and
/// adding its entry here (`docs/serve-push.md`, "Other tools later").
pub const CATALOG: [ToolCatalog; 1] = [ToolCatalog {
    tool: beagle_scout::events::TOOL,
    name: "Scout",
    kinds: &beagle_scout::events::CATALOG,
}];

/// The catalog entry for `tool`.
#[must_use]
pub fn tool(tool: &str) -> Option<&'static ToolCatalog> {
    CATALOG.iter().find(|t| t.tool == tool)
}

/// The kind `kind` of `tool`.
#[must_use]
pub fn kind(tool_name: &str, kind: &str) -> Option<&'static Kind> {
    tool(tool_name)?.kinds.iter().find(|k| k.kind == kind)
}

/// The catalog as the API serves it.
#[must_use]
pub fn catalog_json() -> Value {
    CATALOG
        .iter()
        .map(|t| {
            json!({
                "tool": t.tool,
                "name": t.name,
                "events": t.kinds.iter().map(|k| json!({
                    "kind": k.kind, "label": k.label, "default": k.default, "sound": k.sound,
                })).collect::<Vec<_>>(),
            })
        })
        .collect()
}

/// One tool's settings on one phone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolSettings {
    /// The tool's master switch.
    pub enabled: bool,
    /// One switch per event kind.
    #[serde(default)]
    pub events: BTreeMap<String, bool>,
}

/// A phone's settings: one entry per tool in [`CATALOG`], always complete.
pub type Settings = BTreeMap<String, ToolSettings>;

/// The tool's own defaults: on, with each kind's default.
#[must_use]
pub fn tool_defaults(t: &ToolCatalog) -> ToolSettings {
    ToolSettings {
        enabled: true,
        events: t
            .kinds
            .iter()
            .map(|k| (k.kind.to_owned(), k.default))
            .collect(),
    }
}

/// Every tool's defaults, for a new phone.
#[must_use]
pub fn defaults() -> Settings {
    CATALOG
        .iter()
        .map(|t| (t.tool.to_owned(), tool_defaults(t)))
        .collect()
}

/// `raw` (the body of `PUT …/notifications`) checked against the catalog
/// and completed: a missing tool gets its defaults, a missing kind its
/// default.
///
/// # Errors
/// What is wrong, for a `400`: not an object, an unknown tool or kind, a
/// non-boolean.
pub fn parse(raw: &Value) -> Result<Settings, String> {
    let given: BTreeMap<String, ToolSettings> = serde_json::from_value(raw.clone())
        .map_err(|e| format!("settings must be {{\"<tool>\": {{\"enabled\": bool, \"events\": {{\"<kind>\": bool}}}}}}: {e}"))?;
    let mut settings = defaults();
    for (name, wanted) in given {
        let t = tool(&name).ok_or_else(|| format!("unknown tool {name:?}"))?;
        let entry = settings
            .get_mut(&name)
            .ok_or_else(|| format!("unknown tool {name:?}"))?;
        entry.enabled = wanted.enabled;
        for (k, on) in wanted.events {
            if !t.kinds.iter().any(|kind| kind.kind == k) {
                return Err(format!("unknown {name} event {k:?}"));
            }
            entry.events.insert(k, on);
        }
    }
    Ok(settings)
}

/// `stored` completed against the catalog (a tool or kind added since it
/// was saved gets its default; ones no longer known are dropped).
#[must_use]
pub fn complete(stored: &Settings) -> Settings {
    let mut settings = defaults();
    for (name, entry) in &mut settings {
        if let Some(saved) = stored.get(name) {
            entry.enabled = saved.enabled;
            for (k, on) in &mut entry.events {
                if let Some(saved_on) = saved.events.get(k) {
                    *on = *saved_on;
                }
            }
        }
    }
    settings
}

/// Whether `settings` want `kind` events from `tool` (unknown ones: no).
#[must_use]
pub fn allows(settings: &Settings, tool_name: &str, kind_name: &str) -> bool {
    let Some(k) = kind(tool_name, kind_name) else {
        return false;
    };
    settings.get(tool_name).map_or(k.default, |t| {
        t.enabled && t.events.get(kind_name).copied().unwrap_or(k.default)
    })
}
