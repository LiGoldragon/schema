# schema intent

`schema` exists because schema work now has its own repository. Its purpose
is to give the schema-language and macro work a typed substrate instead of
keeping schema knowledge scattered across design reports and macro internals.

## What this crate is for

This crate owns the parse + lowering substrate of the schema engine. It
reads NOTA `.schema` files, recognises their feature surface, lowers them
into an `AssembledSchema` machine object, and applies the universal
post-passes that every actor schema depends on. It does not emit Rust; the
composer in `signal-frame/schema-rust` does. The boundary is deliberate:
this crate owns parse + lowering only.

## Current intent, in priority order

1. Represent the NOTA schema language as typed Rust data.
2. Keep the top-level `.schema` file as the six fixed positional fields:
   imports, ordinary header, owner header, sema header, namespace, features.
3. Use NOTA's curly-brace map form as the name-value substrate for schema
   namespaces.
4. Keep the schema language positional: type first, then fields in
   declaration order.
5. Support the current Spirit MVP shape: uniform `(Root [SubVariant ...])`
   route headers, namespace-defined route bodies, cross-schema imports,
   root-plus-ordered-box layout, and feature-carried upgrade annotations.
6. Lower authored schemas into `AssembledSchema`, the explicit machine
   object consumed by short-header generation, code emission, storage
   descriptors, and version projection.
7. Test the schema language end-to-end from actual `.schema` files using
   local relative imports before treating the Rust-only model as sufficient.
8. Stay library-shaped until the runtime schema registry/triad authority is
   explicitly settled.

## Multi-pass parser philosophy

The parser walks the NOTA value tree in passes rather than in a single
deep recursion. A canonical `shape_parser` (operating on parsed
`NotaValue` trees) and a streaming `parser` (operating directly on the
token stream) coexist; both produce identical typed outputs. The
multi-pass design pays for itself when features compose: a recognizer for
`EffectTable` can fire independently of `FanOutTargets`, and both run
through the same fixed-point pipeline.

The shape-logic dispatch site (`multi_pass.rs`) routes feature heads to
named recognizers — currently `EffectTableRecognizer`,
`FanOutTargetsRecognizer`, `StorageDescriptorRecognizer`, alongside the
pre-existing builtins. New feature variants land by adding a recognizer
to the dispatch site, a parser entry-point in `shape_parser.rs` and
`parser.rs`, and a typed `Feature` enum variant.

## The three authored Feature variants

Per the schema-driven actor architecture (design captured in
`reports/designer/345/346` of the primary workspace and synthesised into
this repo's substrate):

- `Feature::EffectTable(EffectTableFeature)` — closed mapping from
  ACTION-enum variants to effect type names. Entries are
  `(action_name, effect_type_name)` pairs.
- `Feature::FanOutTargets(FanOutTargetsFeature)` — per-effect closed set
  of fan-out outputs. Each output is one of three closed kinds:
  `FanOutOutputDeclaration::Reply { variant }`, `::Actor { method_tag,
  actor_type, actor_method }`, `::Subscribers { actor_type,
  dispatch_method }`.
- `Feature::StorageDescriptor(StorageDescriptorFeature)` — closed set of
  `(logical_name, table_type)` entries naming the redb tables an actor
  schema owns.

These three variants are what make actor schemas declaratively complete:
the ACTION + RESPONSE enums (declared in the namespace section) plus the
effect-table + fan-out + storage features (declared in the features
section) name everything the composer needs to emit + everything the
hand-written engine needs to consume.

## The universal-Unknown post-pass hook

`LoweringContext::finalize_universal_unknowns()` is the engine's universal
post-pass: it walks the lowered local types after all `TypeMacro`
invocations complete, identifies any local enum whose name ends in
`Response`, and idempotently injects an `Unknown(String)` variant. This
gives every actor schema a structural **safety floor** — no matter what
arrives at an actor, the actor has a structurally-valid response.

Three guards inside `inject_unknown_into_enum_body`:

1. **Enum-shape only.** Record bodies and aliases are left alone.
2. **Idempotent.** Short-circuits if `Unknown` is already present.
3. **Suffix-only matching.** Enums named `*Response` qualify; enums
   containing "Response" mid-string do not.

The hook is wired from BOTH `Schema::assemble` AND the multi-pass
`MacroPipeline::run` so both lowering paths produce identical
`AssembledSchema` outputs.

## Open intent needing later settlement

- Whether this repository also owns the eventual `nota-box` crate surface
  or whether `nota-codec` keeps the wire container and `schema` only owns
  metadata.
- Whether a future schema daemon triad is required before schema metadata
  is queried at runtime.
- Whether schema imports begin as path references only, Cargo symbolic
  references, or both. (Cross-crate import resolution between sibling
  crates is the load-bearing deferral — the persona-spirit schemas
  reference sibling-crate schemas which the current `LoadedSchema::
  read_path` resolver can't follow.)
- Whether upgrade grammar grows beyond `Migrate`, `RenamedFrom`, `Drop`,
  `Custom`, and `Untranslatable` before the Spirit pilot needs it.
