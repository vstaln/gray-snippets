//! gray-snippets — named prompt snippets with inline @name expansion.
//!
//! `/snip save|del|list|show` manages snippets stored at
//! `~/.gray/snippets/snippets.json`. The `input/submit` hook rewrites
//! `@name` references into the snippet body; `prompt/context` advertises
//! the defined names so the model knows the macros exist.

use std::collections::BTreeMap;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

const NAME: &str = "snippets";

fn manifest() -> Value {
    json!({
        "name": NAME,
        "display_name": "Snippets",
        "version": env!("CARGO_PKG_VERSION"),
        "protocol": "1.1",
        "tools": [],
        "commands": ["/snip"],
        "hooks": ["input/submit", "prompt/context"],
    })
}

/// `<gray-home>/snippets/snippets.json` — honors `$GRAY_HOME`, falls back
/// to `$HOME/.gray`.
fn store_path() -> PathBuf {
    std::env::var_os("GRAY_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".gray")))
        .unwrap_or_else(|| PathBuf::from(".gray"))
        .join(NAME)
        .join("snippets.json")
}

fn load_from(path: &Path) -> BTreeMap<String, String> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn store_at(path: &Path, snips: &BTreeMap<String, String>) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_string_pretty(snips).unwrap())
        .map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, path).map_err(|e| e.to_string())
}

fn load() -> BTreeMap<String, String> {
    load_from(&store_path())
}

fn store(snips: &BTreeMap<String, String>) -> Result<(), String> {
    store_at(&store_path(), snips)
}

fn is_name_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '-' || c == '_'
}

fn valid_name(n: &str) -> bool {
    !n.is_empty() && n.chars().all(is_name_char)
}

/// Rewrite `@name` tokens that match a snippet. Single pass: bodies are
/// inserted verbatim and never re-scanned. `@@` is an escaped literal
/// `@`. Returns the rewritten text, or None when nothing expanded so the
/// caller can pass the original through untouched.
fn expand(text: &str, snips: &BTreeMap<String, String>) -> Option<String> {
    let mut out = String::with_capacity(text.len());
    let mut changed = false;
    let mut rest = text;
    while let Some(at) = rest.find('@') {
        out.push_str(&rest[..at]);
        let tail = &rest[at + 1..];
        if tail.starts_with('@') {
            out.push('@');
            rest = &tail[1..];
            changed = true;
            continue;
        }
        let n = tail.find(|c: char| !is_name_char(c)).unwrap_or(tail.len());
        let name = &tail[..n];
        if let Some(body) = snips.get(name) {
            out.push_str(body);
            changed = true;
        } else {
            out.push('@');
            out.push_str(name);
        }
        rest = &tail[n..];
    }
    out.push_str(rest);
    changed.then_some(out)
}

fn usage() -> String {
    "usage: snip save <name> <text> | del <name> | list | show <name>".into()
}

fn cli(argv: &[String]) -> Result<String, String> {
    let mut snips = load();
    match argv.first().map(String::as_str) {
        Some("save") => {
            let name = argv.get(1).ok_or_else(usage)?;
            if !valid_name(name) {
                return Err(format!("bad name {name:?} — use [A-Za-z0-9_-]+"));
            }
            let body = argv[2..].join(" ");
            if body.trim().is_empty() {
                return Err("usage: snip save <name> <text>".into());
            }
            snips.insert(name.to_string(), body);
            store(&snips)?;
            Ok(format!(
                "saved @{name} ({} chars)",
                snips[name].chars().count()
            ))
        }
        Some("del") | Some("rm") | Some("delete") => {
            let name = argv.get(1).ok_or_else(usage)?;
            if snips.remove(name).is_none() {
                return Ok(format!("no snippet named {name}"));
            }
            store(&snips)?;
            Ok(format!("deleted @{name}"))
        }
        Some("show") => {
            let name = argv.get(1).ok_or_else(usage)?;
            match snips.get(name) {
                Some(b) => Ok(b.clone()),
                None => Ok(format!("no snippet named {name}")),
            }
        }
        Some("list") | None => {
            if snips.is_empty() {
                return Ok("no snippets — /snip save <name> <text>".into());
            }
            Ok(snips
                .iter()
                .map(|(k, v)| format!("@{k} ({} chars)", v.chars().count()))
                .collect::<Vec<_>>()
                .join("\n"))
        }
        Some(_) => Err(usage()),
    }
}

fn sidecar() -> std::io::Result<()> {
    let stdin = std::io::stdin();
    let mut out = std::io::stdout().lock();
    for line in stdin.lock().lines() {
        let line = line?;
        let Ok(v) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        let method = v["method"].as_str().unwrap_or("");
        if method == "plugin/shutdown" {
            if let Some(id) = v.get("id") {
                writeln!(out, "{}", json!({"id": id, "result": {}}))?;
                out.flush()?;
            }
            return Ok(());
        }
        match v.get("id").cloned() {
            None => {}
            Some(id) => {
                let reply = if method == "plugin/manifest" {
                    json!({"id": id, "result": manifest()})
                } else if method == "input/submit" {
                    let text = v["params"]["text"].as_str().unwrap_or("");
                    match expand(text, &load()) {
                        Some(t) => json!({"id": id, "result": {"text": t}}),
                        None => json!({"id": id, "result": {}}),
                    }
                } else if method == "prompt/context" {
                    let snips = load();
                    if snips.is_empty() {
                        json!({"id": id, "result": {}})
                    } else {
                        let names = snips
                            .keys()
                            .map(|k| format!("@{k}"))
                            .collect::<Vec<_>>()
                            .join(", ");
                        json!({"id": id, "result": {"text": format!(
                            "Prompt snippets — @name in user input expands inline: {names}")}})
                    }
                } else if method == "command/run" {
                    let argv: Vec<String> = v["params"]["argv"]
                        .as_array()
                        .map(|a| {
                            a.iter()
                                .filter_map(Value::as_str)
                                .map(str::to_string)
                                .collect()
                        })
                        .unwrap_or_default();
                    let text = cli(&argv).unwrap_or_else(|e| e);
                    json!({"id": id, "result": {"text": text}})
                } else {
                    json!({"id": id, "error": {"code": -32601,
                        "message": format!("method not found: {method}")}})
                };
                writeln!(out, "{reply}")?;
                out.flush()?;
            }
        }
    }
    Ok(())
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<String> = match args.first().map(String::as_str) {
        Some("snip") | Some("snippets") => args[1..].to_vec(),
        _ => args,
    };
    match args.first().map(String::as_str) {
        Some("manifest") => println!("{}", manifest()),
        Some("sidecar") => {
            let _ = sidecar();
        }
        _ => {
            if args.is_empty() {
                if std::io::IsTerminal::is_terminal(&std::io::stdin()) {
                    println!("snippets — {}", usage());
                    return;
                }
                let _ = sidecar();
                return;
            }
            match cli(&args) {
                Ok(t) => println!("{t}"),
                Err(e) => {
                    eprintln!("{e}");
                    std::process::exit(1);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snips(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn manifest_shape() {
        let m = manifest();
        assert_eq!(m["name"], "snippets");
        assert_eq!(m["commands"], json!(["/snip"]));
        assert_eq!(m["hooks"], json!(["input/submit", "prompt/context"]));
    }

    #[test]
    fn expands_known_name() {
        let s = snips(&[("py", "use python3")]);
        assert_eq!(
            expand("review @py please", &s).as_deref(),
            Some("review use python3 please")
        );
    }

    #[test]
    fn leaves_unknown_and_email() {
        let s = snips(&[("x", "X")]);
        assert_eq!(expand("mail a@b.com", &s), None);
        assert_eq!(expand("@nope here", &s), None);
    }

    #[test]
    fn double_at_escapes() {
        let s = snips(&[("x", "X")]);
        assert_eq!(expand("@@x", &s).as_deref(), Some("@x"));
        assert_eq!(
            expand("keep @@ literal", &s).as_deref(),
            Some("keep @ literal")
        );
    }

    #[test]
    fn no_partial_prefix_expansion() {
        let s = snips(&[("fo", "FO")]);
        assert_eq!(expand("see @foo", &s), None);
    }

    #[test]
    fn name_rules() {
        assert!(valid_name("a-b_c9"));
        assert!(!valid_name(""));
        assert!(!valid_name("a b"));
        assert!(!valid_name("@x"));
    }

    #[test]
    fn store_roundtrip() {
        let dir = std::env::temp_dir().join(format!(
            "gray-snippets-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let path = dir.join("snippets.json");
        let s = snips(&[("one", "1"), ("two", "2")]);
        store_at(&path, &s).unwrap();
        assert_eq!(load_from(&path), s);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
