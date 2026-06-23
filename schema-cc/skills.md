# schema-cc — skill index

Read for working in this repo, in order:

- `INTENT.md`, `ARCHITECTURE.md` (this repo) — what schema-cc is, how it is built,
  the three-tier layering and the bootstrap.
- primary `skills/structural-forms.md` — the "a language is data" concept and the
  shape vocabulary schema-cc's grammar generates from.
- primary `skills/rust-discipline.md` and its `rust/*.md` sub-files
  (`methods.md`, `errors.md`, `parsers.md`, `crate-layout.md`,
  `storage-and-wire.md`), plus `skills/abstractions.md` — **required before
  authoring or editing Rust here.**
- Upstream/downstream intent: `nota/INTENT.md` (the seed schema-cc decodes
  through) and `schema/INTENT.md` (the compiler schema-cc generates into).
- Designer reports `649` (precedence-as-data decision, Spirit `549v`) and `652`
  (schema-cc design, leans, and roadmap); operator review `384`.
