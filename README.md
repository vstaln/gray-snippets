<div align="center">
  <img alt="gray-snippets" src="assets/icon.svg" width="120" height="120" />
  <h1>gray-snippets</h1>
  <p><strong>Named prompt snippets with inline <code>@name</code> expansion.</strong></p>
  <p>
    <a href="https://gray.alignment.id">Website</a> ·
    <a href="https://gray.alignment.id/plugins/gray-snippets">Store</a> ·
    <a href="https://github.com/vstaln/gray-snippets">Source</a> ·
    <a href="https://github.com/vstaln/gray">gray</a>
  </p>
  <p>
    <a href="LICENSE"><img alt="License: MIT" src="https://img.shields.io/badge/license-MIT-1c1c20?style=flat-square&labelColor=0a0a0b" /></a>
    <a href="https://www.rust-lang.org"><img alt="Built with Rust" src="https://img.shields.io/badge/built%20with-rust-1c1c20?style=flat-square&labelColor=0a0a0b&logo=rust&logoColor=d4a373" /></a>
    <a href="https://gray.alignment.id/plugins/gray-snippets"><img alt="gray plugin" src="https://img.shields.io/badge/gray-plugin-1c1c20?style=flat-square&labelColor=0a0a0b&color=7aa2f7" /></a>
  </p>
</div>

<br/>

```bash
gray plugin install gray-snippets
```

Save reusable prompt fragments once, then drop them into any message with
`@name`. The `input/submit` hook expands the references inline before your
prompt is submitted.

## Commands

```text
/snip save <name> <text>   save a snippet (name: [A-Za-z0-9_-]+)
/snip list                 list all snippets with sizes
/snip show <name>          print a snippet body
/snip del <name>           delete a snippet
```

## Expansion

On `input/submit`, every `@name` token that matches a saved snippet is
replaced with its body — single pass, bodies are inserted verbatim and
never re-scanned. Unmatched `@tokens` (emails, handles) are left alone,
and `@@` escapes to a literal `@`. If nothing expands, the hook returns
`{}` and your input goes through untouched.

`prompt/context` advertises the defined names — `@a, @b, @c` — so the
model knows the macros exist; it returns `{}` when no snippets are saved.

## State

Snippets live in `~/.gray/snippets/snippets.json` (`$GRAY_HOME` honored,
`$HOME/.gray` fallback) — a plain `{name: body}` object, easy to edit or
sync by hand.
