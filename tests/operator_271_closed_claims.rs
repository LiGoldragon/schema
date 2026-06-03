//! Architectural-truth witnesses for the closed claims in operator 271
//! `reports/operator/271-context-maintenance-current-state-2026-06-01.md`.
//!
//! Each witness proves the closure named in the report against the current
//! state of the schema-next sources. The tests are positive witnesses: they
//! assert the present shape of the code, types, and fixtures. If a future
//! agent reverts any of the closures, the test fails.
//!
//! Coverage:
//! - Claim 1 — macro-library source/artifact datatype split CLOSED
//!   (schema-next `99078b20`).
//! - Claim 4 — strict schema syntax and honest enum bodies CLOSED.
//! - Claim 5 — Asschema as typed data with NOTA, rkyv, and SEMA projection
//!   CLOSED.
//!
//! Companion witnesses live in:
//! - `tests/asschema_definition.rs` — round-trip, store, artifact tests
//!   (claim 5 substrate).
//! - `tests/macro_exploration.rs::retired_duplicate_macro_datatype_names_do_not_return`
//!   — negative-witness guard for claim 1.
//! - The flake's `library-mirrors-collapsed` check — Nix-side regression
//!   guard for claim 1.

use std::path::Path;

use nota_next::{Block, Delimiter, Document};
use schema_next::{
    Asschema, AsschemaArtifact, AsschemaStore, MacroLibrary, MacroLibraryArtifact, SchemaEngine,
    SchemaIdentity, SchemaMacro, TypeDeclaration,
};

/// Claim 1 — `MacroLibrary` is one type, not split between source and
/// artifact mirrors. The library's source-entries field is named
/// `source_entries: Vec<MacroLibrarySourceEntry>` (the rename happened in
/// the `99078b20` collapse) and the only present variant is `SchemaMacro`.
#[test]
fn macro_library_source_entries_are_one_type() {
    let source = include_str!("../schemas/builtin-macros.macro-library");
    let library = MacroLibrary::from_nota_source(source)
        .expect("checked-in builtin macro library decodes through one MacroLibrary type");

    assert!(
        !library.source_entries().is_empty(),
        "builtin library carries source entries through MacroLibrary::source_entries"
    );

    for entry in library.source_entries() {
        // The variant_name() method names which enum case the entry holds.
        // After the collapse, only `SchemaMacro` is present — there is no
        // sibling `MacroLibrarySourceEntryData` enum behind the scenes.
        assert_eq!(
            entry.variant_name(),
            "SchemaMacro",
            "the only source-entry variant after the collapse is SchemaMacro"
        );
        // The definition accessor returns `&SchemaMacro` directly, not a
        // separate `MacroDefinitionData` mirror.
        let _macro_definition: &SchemaMacro = entry.definition();
    }
}

/// Claim 1 — `MacroLibraryArtifact` wraps `MacroLibrary` and is the only
/// projection noun for the artifact concern. The previous split between
/// `DeclarativeMacroLibrary` and `MacroLibraryData` no longer exists in the
/// public surface.
#[test]
fn macro_library_artifact_wraps_the_one_library_type() {
    let source = include_str!("../schemas/builtin-macros.macro-library");
    let artifact = MacroLibraryArtifact::from_nota_source(source)
        .expect("checked-in builtin library decodes as artifact");

    // The artifact projects to and from NOTA + rkyv through the same one
    // library type — no Data mirror is required to traverse the boundary.
    let nota = artifact.to_nota_source();
    let from_nota = MacroLibraryArtifact::from_nota_source(&nota)
        .expect("artifact NOTA round-trips through one library type");
    assert_eq!(artifact.library(), from_nota.library());

    let bytes = artifact
        .to_binary_bytes()
        .expect("artifact archives through rkyv");
    let from_binary =
        MacroLibraryArtifact::from_binary_bytes(&bytes).expect("artifact decodes from rkyv bytes");
    assert_eq!(artifact.library(), from_binary.library());

    // `into_library()` consumes the artifact into the inner library noun.
    // The conversion does not pass through any intermediate Data type.
    let library: MacroLibrary = artifact.into_library();
    assert!(!library.source_entries().is_empty());
}

/// Claim 1 — Source-AST witness that the legacy split names are absent from
/// the public surface of the library code. This complements the existing
/// guard in `tests/macro_exploration.rs::retired_duplicate_macro_datatype_names_do_not_return`
/// by scanning the `pub use` re-export in `lib.rs` and the type declarations
/// at the head of `declarative.rs`.
#[test]
fn macro_library_split_does_not_return_through_public_surface() {
    let lib_rs = include_str!("../src/lib.rs");
    let declarative_rs = include_str!("../src/declarative.rs");

    // The collapse removed these as PUBLIC types; the regression guard at
    // tests/macro_exploration.rs:400 covers the broader file, but the
    // tightest signal is that the `pub use` line for declarative no longer
    // contains the retired Data names.
    let pub_use_block = lib_rs
        .lines()
        .skip_while(|line| !line.contains("pub use declarative::"))
        .take_while(|line| !line.trim().ends_with("};"))
        .collect::<Vec<_>>()
        .join("\n");

    let retired_public_names = [
        "DeclarativeMacroLibrary",
        "MacroLibraryData",
        "MacroLibrarySourceEntryData",
        "MacroDefinitionData",
        "MacroPatternData",
        "MacroTemplateData",
    ];
    for retired in retired_public_names {
        assert!(
            !pub_use_block.contains(retired),
            "schema-next lib.rs must not re-export retired split name {retired}"
        );
    }

    // The current `pub use` line MUST carry the present shape's names.
    assert!(
        pub_use_block.contains("MacroLibrary,") || pub_use_block.contains("MacroLibrary\n"),
        "schema-next lib.rs must re-export MacroLibrary as the one type"
    );
    assert!(
        pub_use_block.contains("MacroLibraryArtifact"),
        "schema-next lib.rs must re-export MacroLibraryArtifact"
    );
    assert!(
        pub_use_block.contains("MacroLibrarySourceEntry,")
            || pub_use_block.contains("MacroLibrarySourceEntry\n"),
        "schema-next lib.rs must re-export MacroLibrarySourceEntry"
    );

    // The declarative source declares the present canonical shape.
    assert!(
        declarative_rs.contains("pub struct MacroLibrary {"),
        "declarative.rs must declare pub struct MacroLibrary"
    );
    assert!(
        declarative_rs.contains("pub struct MacroLibraryArtifact {"),
        "declarative.rs must declare pub struct MacroLibraryArtifact"
    );
    assert!(
        declarative_rs.contains("pub enum MacroLibrarySourceEntry {"),
        "declarative.rs must declare pub enum MacroLibrarySourceEntry"
    );
    assert!(
        declarative_rs.contains("source_entries: Vec<MacroLibrarySourceEntry>"),
        "MacroLibrary must hold source_entries: Vec<MacroLibrarySourceEntry>"
    );
    assert!(
        declarative_rs.contains("library: MacroLibrary"),
        "MacroLibraryArtifact must hold library: MacroLibrary"
    );
    assert!(
        declarative_rs.contains("SchemaMacro(SchemaMacro)"),
        "MacroLibrarySourceEntry::SchemaMacro(SchemaMacro) is the canonical variant"
    );
}

/// Claim 4 — Strict schema syntax: the production `core.schema` and
/// `spirit-min.schema` carry honest parenthesized enum-body data variants
/// like `(Record Entry)` plus bare PascalCase unit variants. The retired
/// `Record@Entry` short-suffix sugar must not appear.
#[test]
fn production_schema_sources_use_honest_enum_bodies() {
    let core_schema = include_str!("../schemas/core.schema");
    let spirit_min_schema = include_str!("../schemas/spirit-min.schema");
    let root_schema = include_str!("../schemas/root.schema");
    let builtin_macros_schema = include_str!("../schemas/builtin-macros.schema");

    for (name, source) in [
        ("core.schema", core_schema),
        ("spirit-min.schema", spirit_min_schema),
        ("root.schema", root_schema),
        ("builtin-macros.schema", builtin_macros_schema),
    ] {
        // No retired `@` short-suffix variant sugar.
        // Allowed `@` use: none in schema files. The check is total.
        assert!(
            !source.contains('@'),
            "{name} must not carry the retired `@` short-suffix sugar"
        );

        // Each schema must parse as legal NOTA — proves the honest bodies
        // are syntactically valid through the same parser the engine uses.
        Document::parse(source).unwrap_or_else(|error| {
            panic!("{name} must parse as legal NOTA (honest bodies are NOTA-valid): {error}")
        });
    }
}

/// Claim 4 — Spirit-min carries enum bodies of the explicit
/// `[(Record Entry) (Observe Query)]` shape, with each element being a
/// parenthesized data-variant record. The first root object (input) holds
/// at least two variants of this shape.
#[test]
fn spirit_min_input_enum_body_has_parenthesized_data_variants() {
    let source = include_str!("../schemas/spirit-min.schema");
    let document = Document::parse(source).expect("spirit-min.schema parses as NOTA");
    let root_objects = document.root_objects();

    let input = root_objects
        .first()
        .expect("spirit-min schema starts with an input enum-body vector");
    let Block::Delimited {
        delimiter,
        root_objects: variants,
        ..
    } = input
    else {
        panic!("input root must be a delimited block")
    };
    assert_eq!(
        *delimiter,
        Delimiter::SquareBracket,
        "input is a SquareBracket enum-body vector"
    );

    // Every element of the vector is either a bare atom (unit variant) or
    // a parenthesized record (data variant). spirit-min carries only data
    // variants, so each element MUST be a parenthesized record.
    assert!(
        !variants.is_empty(),
        "input vector contains at least one variant"
    );
    for variant in variants {
        match variant {
            Block::Delimited {
                delimiter: Delimiter::Parenthesis,
                ..
            } => { /* parenthesized data variant; expected shape */ }
            _ => panic!(
                "every spirit-min input variant must be parenthesized data variant; got {variant:?}"
            ),
        }
    }
}

/// Claim 5 — `Asschema` is typed Rust data. The type carries the schema
/// identity plus the typed projections of imports, resolved imports, input,
/// output, and namespace declarations. This is the noun the rest of the
/// projection chain consumes.
#[test]
fn asschema_is_typed_data_with_named_field_accessors() {
    let source = include_str!("../schemas/core.schema");
    let asschema: Asschema = SchemaEngine::default()
        .lower_source(source, SchemaIdentity::new("schema-next:core", "0.1.0"))
        .expect("core schema lowers to typed Asschema data");

    assert_eq!(asschema.identity().component().as_str(), "schema-next:core");
    assert_eq!(asschema.identity().version(), "0.1.0");

    // Typed accessors — Asschema is a noun with methods, not a string blob.
    let _: &[schema_next::ImportDeclaration] = asschema.imports();
    let _: &schema_next::EnumDeclaration = asschema.input();
    let _: &schema_next::EnumDeclaration = asschema.output();
    let _: &[schema_next::Declaration] = asschema.namespace();

    // The namespace carries typed `Declaration` values; pick one and
    // confirm it lowers into one of the typed variants of `TypeDeclaration`.
    let any_declaration = asschema
        .namespace()
        .first()
        .expect("core schema has at least one namespace declaration");
    match any_declaration.value() {
        TypeDeclaration::Alias(_)
        | TypeDeclaration::Struct(_)
        | TypeDeclaration::Enum(_)
        | TypeDeclaration::Newtype(_) => { /* typed variant; expected */ }
    }
}

/// Claim 5 — `Asschema` projects to NOTA text and rkyv bytes; both
/// projections round-trip. This is the same shape `tests/asschema_definition.rs`
/// covers more broadly. This witness uses the smaller `core.schema` so the
/// chain is fast and the assertion is precise.
#[test]
fn asschema_round_trips_through_nota_and_rkyv() {
    let source = include_str!("../schemas/core.schema");
    let asschema = SchemaEngine::default()
        .lower_source(source, SchemaIdentity::new("schema-next:core", "0.1.0"))
        .expect("core schema lowers");

    // NOTA projection round-trips.
    let nota = asschema.to_nota();
    let from_nota = Asschema::from_nota_source(&nota).expect("emitted NOTA decodes back");
    assert_eq!(from_nota, asschema);

    // rkyv projection round-trips.
    let bytes = asschema
        .to_binary_bytes()
        .expect("asschema serialises to rkyv bytes");
    let from_bytes =
        Asschema::from_binary_bytes(&bytes).expect("rkyv bytes decode back to Asschema");
    assert_eq!(from_bytes, asschema);

    // The artifact projection holds the same data behind a separate noun.
    let artifact = AsschemaArtifact::new(asschema.clone());
    let artifact_bytes = artifact
        .to_binary_bytes()
        .expect("artifact serialises through rkyv");
    let recovered = AsschemaArtifact::from_binary_bytes(&artifact_bytes)
        .expect("artifact decodes from rkyv bytes");
    assert_eq!(recovered.asschema(), &asschema);
}

/// Claim 5 — `AsschemaStore` is the SEMA persistence noun: it writes
/// archived rkyv bytes into redb, keyed by `SchemaIdentity`, and exports
/// the stored schema back to NOTA. The store is a real durable substrate,
/// not a side-effect-free wrapper.
#[test]
fn asschema_store_persists_through_redb_and_reexports_nota() {
    let source = include_str!("../schemas/core.schema");
    let asschema = SchemaEngine::default()
        .lower_source(source, SchemaIdentity::new("schema-next:core", "0.1.0"))
        .expect("core schema lowers");

    let store_directory = std::env::temp_dir().join(format!(
        "schema-next-operator-271-store-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&store_directory);
    std::fs::create_dir_all(&store_directory).expect("create store directory");

    let store_path = store_directory.join("schemas.sema");
    let store = AsschemaStore::open(&store_path).expect("AsschemaStore opens at the chosen path");

    assert!(store.is_empty().expect("fresh store is readable"));
    store
        .put_asschema(&asschema)
        .expect("put_asschema writes through the SEMA-storage path");
    assert_eq!(store.len().expect("store length is queryable"), 1);

    let recovered = store
        .get_asschema(asschema.identity())
        .expect("store reads back without error")
        .expect("the stored schema is present");
    assert_eq!(recovered, asschema);

    // export_nota_file projects the stored archived value back to NOTA text.
    let export_path = store_directory.join("exported.asschema");
    store
        .export_nota_file(asschema.identity(), &export_path)
        .expect("store exports back to NOTA text");
    let exported = std::fs::read_to_string(&export_path).expect("read exported NOTA file");
    let from_export =
        Asschema::from_nota_source(&exported).expect("exported NOTA decodes as Asschema");
    assert_eq!(from_export, asschema);

    drop(store);

    // The on-disk file is a real redb database — the path exists after the
    // store is dropped. This is the SEMA-persistence witness.
    assert!(
        store_path.exists(),
        "the SEMA database file persists after the store is dropped"
    );

    let _ = std::fs::remove_dir_all(&store_directory);
}

/// Claim 5 — The checked-in `core.asschema` artifact is FRESH against the
/// authored `core.schema`. This is the artifact-discipline claim per
/// `skills/designer.md` §"Audit precision": the durable `.asschema` file
/// is the emitter's first-class input, not just a round-trip capability.
#[test]
fn checked_in_core_asschema_artifact_matches_lowered_schema() {
    let lowered = SchemaEngine::default()
        .lower_source(
            include_str!("../schemas/core.schema"),
            SchemaIdentity::new("schema-next:core", "0.1.0"),
        )
        .expect("core schema lowers");
    let checked_in_text = include_str!("../schemas/core.asschema");
    let checked_in = AsschemaArtifact::from_nota_source(checked_in_text)
        .expect("checked-in core.asschema decodes");

    assert_eq!(
        checked_in.asschema(),
        &lowered,
        "schemas/core.asschema must be refreshed when schema or lowering changes"
    );

    // The checked-in artifact is also a real file (not just an
    // include_str path). This confirms the artifact discipline.
    let schemas_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("schemas");
    assert!(
        schemas_dir.join("core.asschema").exists(),
        "schemas/core.asschema is a real checked-in file"
    );
}
