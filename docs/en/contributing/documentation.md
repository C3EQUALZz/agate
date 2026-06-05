# Documentation Guide

**Every feature must be documented.** A change is not done until the docs
reflect it. This page explains how the docs site is built and how to add or
update pages — including keeping the **English** and **Russian** versions in
sync.

## Stack

The site is built with **[Material for MkDocs](https://squidfunk.github.io/mkdocs-material/)**
plus **[mkdocs-static-i18n](https://github.com/ultrabug/mkdocs-static-i18n)** for
bilingual content. Configuration is in `mkdocs.yml` at the repository root;
the pinned toolchain is in `docs/requirements.txt`.

!!! note "Why not Zensical?"
    The Material for MkDocs team's TOML-configured successor, **Zensical**, was
    considered (the maintainer referenced it). As of 2026 its multi-language
    *content* workflow is on the roadmap but not yet implemented, so it cannot
    yet maintain parallel EN/RU page trees with a language switcher. Bilingual
    docs are a hard requirement, so we use the mature Material + i18n stack
    (the same stack FastStream and AG2 use). Migration to Zensical later is a
    supported, near-mechanical path.

## Build and preview locally

```bash
python -m pip install -r docs/requirements.txt
mkdocs serve          # live-reloading preview at http://127.0.0.1:8000
mkdocs build --strict # what CI runs; fails on warnings (broken links, etc.)
```

## Folder layout

Docs use a **folder-based** bilingual structure: each language has its own
mirrored tree under `docs/`.

```text
docs/
  en/                         # English — the primary, authoritative tree
    index.md
    getting-started/
    architecture/
      contexts/
    reference/
    contributing/
  ru/                         # Russian — mirrors en/; untranslated pages fall back to en/
  design/                     # in-repo design records (e.g. the threat model)
  requirements.txt
```

- `docs/en/**` is **authoritative**. Write or update English first.
- `docs/ru/**` mirrors the same paths. A page present in `en/` but missing in
  `ru/` **falls back** to English automatically (`fallback_to_default: true`),
  so the site never 404s on an untranslated page.

## Adding or changing a page

1. **Edit/create the English page** under `docs/en/...`.
2. If it is a new page, add it to the `nav:` in `mkdocs.yml` (paths are relative
   to a language folder, e.g. `getting-started/configuration.md`).
3. If the nav entry is a new section title, add its Russian translation under
   the `i18n` plugin's `nav_translations.ru` map in `mkdocs.yml`.
4. **Mirror the page** at the same path under `docs/ru/...` with the translated
   content. If you cannot translate yet, **skip the RU file** — fallback keeps
   the site valid — but open a follow-up so the RU tree catches up.
5. Run `mkdocs build --strict` and fix any warnings.

## Keeping EN and RU in sync

The practical workflow:

- **English is the source of truth.** Every content change lands in `en/` first.
- The Russian mirror is allowed to lag; **fallback prevents broken pages**.
- Track translation debt: when an `en/` page changes, the corresponding `ru/`
  page is stale until updated. Mark stale RU pages with an admonition at the top
  and/or track them in the PR description so they are not forgotten.
- Prefer **small, page-scoped PRs** so an EN change and its RU translation can
  land together.

!!! tip "Diagrams and code are shared, not translated"
    Mermaid diagrams, code blocks, identifiers, env-var names, and CLI commands
    are identical across languages — only prose is translated. This keeps the
    two trees cheap to keep in sync.

## Authoring features available

- **Admonitions:** `!!! note`, `!!! warning`, `!!! tip`, `!!! info`, `!!! abstract`.
- **Collapsible blocks:** `??? note`.
- **Content tabs:** ```` === "Tab" ````.
- **Mermaid diagrams:** fenced ```` ```mermaid ```` blocks.
- **Code highlighting + copy button:** fenced code blocks with a language tag.
- **Snippet includes:** `--8<-- "path"` (used by the
  [Threat Model](../architecture/threat-model.md) page to pull in the in-repo
  design record so it never drifts).
