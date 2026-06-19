use std::fs;

use schema_next::{
    ImplFact, ImplReference, MethodParameter, MethodSignature, Name, RustSurface, Schema,
    SchemaEngine, SchemaError, SchemaIdentity, SchemaSourceArtifact, SourceImplEntry,
    SourceNamespaceEntry, SourceReference, TypeDeclaration, TypeReference,
};

fn impl_catalog_fixture(name: &str) -> String {
    fs::read_to_string(format!("tests/fixtures/impl-catalog/{name}.schema"))
        .unwrap_or_else(|error| panic!("read impl-catalog schema fixture {name}: {error}"))
        .trim_end()
        .to_owned()
}

fn namespace_entries(artifact: &SchemaSourceArtifact) -> Vec<SourceNamespaceEntry> {
    artifact.source().namespace().entries().to_vec()
}

/// Lower a fixture through the typed source archive into a `Schema`, the
/// path that carries the full impl catalog onto each `Declaration` and the
/// standalone `ImplBlock`s.
fn lower_fixture(name: &str) -> Schema {
    let artifact = SchemaSourceArtifact::from_schema_text(&impl_catalog_fixture(name))
        .expect("source decodes");
    SchemaEngine::default()
        .lower_schema_source(artifact.source(), SchemaIdentity::new("example", "0.1.0"))
        .unwrap_or_else(|error| panic!("lower impl-catalog fixture {name}: {error}"))
}

/// The canonical schema-source text must be byte-stable through
/// decode -> to_schema_text -> re-decode, with the `{| … |}` impl block
/// surfaced verbatim. This is the same round-trip contract the source codec
/// tests assert, extended to the new trailing impl-block syntax.
#[test]
fn fused_marker_impls_round_trip() {
    let source = impl_catalog_fixture("fused-markers");
    let artifact = SchemaSourceArtifact::from_schema_text(&source).expect("schema source decodes");
    let canonical = artifact.to_schema_text();

    assert_eq!(
        canonical, source,
        "fused marker impls should write a byte-stable canonical surface"
    );
    assert!(
        canonical.contains("RecordIdentifier String {| Display Ord |}"),
        "canonical surface must carry the fused marker impl block: {canonical}"
    );

    let recovered =
        SchemaSourceArtifact::from_schema_text(&canonical).expect("canonical source decodes");
    assert_eq!(
        artifact, recovered,
        "canonical schema source should recover the same source object"
    );
}

#[test]
fn body_optional_impls_round_trip() {
    let source = impl_catalog_fixture("body-optional");
    let artifact = SchemaSourceArtifact::from_schema_text(&source).expect("schema source decodes");
    let canonical = artifact.to_schema_text();

    assert_eq!(
        canonical, source,
        "body-optional impls should write a byte-stable canonical surface"
    );
    assert!(
        canonical.contains("StatementText {| Display (word_count {} Integer) |}"),
        "canonical surface must carry the body-optional impl block: {canonical}"
    );

    let recovered =
        SchemaSourceArtifact::from_schema_text(&canonical).expect("canonical source decodes");
    assert_eq!(artifact, recovered);
}

#[test]
fn trait_method_signature_impls_round_trip() {
    let source = impl_catalog_fixture("trait-method-sigs");
    let artifact = SchemaSourceArtifact::from_schema_text(&source).expect("schema source decodes");
    let canonical = artifact.to_schema_text();

    assert_eq!(
        canonical, source,
        "trait + method-signature impls should write a byte-stable canonical surface"
    );
    assert!(
        canonical.contains("{| QueryMatcher [ (matches { candidate.Node } Boolean) ] |}"),
        "canonical surface must carry the trait impl with method signatures: {canonical}"
    );

    let recovered =
        SchemaSourceArtifact::from_schema_text(&canonical).expect("canonical source decodes");
    assert_eq!(artifact, recovered);
}

/// The new typed impl-catalog nouns must survive the rkyv archive boundary —
/// the same binary round-trip the source codec asserts for every typed
/// source noun. This is what proves `SourceImplCatalog` / `SourceImplEntry` /
/// `SourceMethodSignature` are real archive members, not parser-only state.
#[test]
fn impl_catalog_round_trips_through_binary_archive() {
    for name in ["fused-markers", "body-optional", "trait-method-sigs"] {
        let source = impl_catalog_fixture(name);
        let artifact =
            SchemaSourceArtifact::from_schema_text(&source).expect("schema source decodes");
        let bytes = artifact
            .to_binary_bytes()
            .expect("schema source artifact archives");
        let recovered = SchemaSourceArtifact::from_binary_bytes(&bytes)
            .expect("schema source artifact restores");

        assert_eq!(artifact, recovered, "binary round-trip for {name}");
        assert_eq!(
            recovered.to_schema_text(),
            source,
            "binary-restored {name} should re-emit the canonical surface"
        );
    }
}

/// The decoded catalog must expose its entries as typed data: markers as
/// trait names, trait impls with their method signatures, and inherent
/// method signatures with typed return references.
#[test]
fn impl_catalog_decodes_each_entry_kind() {
    let fused = SchemaSourceArtifact::from_schema_text(&impl_catalog_fixture("fused-markers"))
        .expect("schema source decodes");
    let entries = namespace_entries(&fused);
    let [record_identifier] = entries.as_slice() else {
        panic!("expected one namespace entry, found {}", entries.len());
    };
    let markers = record_identifier.impls().entries();
    assert_eq!(markers.len(), 2, "two marker impls");
    assert!(matches!(&markers[0], SourceImplEntry::Marker(name) if name.as_str() == "Display"));
    assert!(matches!(&markers[1], SourceImplEntry::Marker(name) if name.as_str() == "Ord"));

    let body_optional =
        SchemaSourceArtifact::from_schema_text(&impl_catalog_fixture("body-optional"))
            .expect("schema source decodes");
    let entries = namespace_entries(&body_optional);
    let [statement_text] = entries.as_slice() else {
        panic!("expected one namespace entry, found {}", entries.len());
    };
    let catalog = statement_text.impls().entries();
    assert_eq!(catalog.len(), 2, "one marker plus one inherent method");
    assert!(matches!(&catalog[0], SourceImplEntry::Marker(name) if name.as_str() == "Display"));
    let SourceImplEntry::InherentMethod(signature) = &catalog[1] else {
        panic!("expected an inherent method, found {:?}", catalog[1]);
    };
    assert_eq!(signature.name().as_str(), "word_count");
    assert!(signature.parameters().is_empty(), "nullary method");
    assert!(
        matches!(signature.return_reference(), SourceReference::Plain(name) if name.as_str() == "Integer"),
        "return reference resolves to Integer"
    );

    let trait_sigs =
        SchemaSourceArtifact::from_schema_text(&impl_catalog_fixture("trait-method-sigs"))
            .expect("schema source decodes");
    let entries = namespace_entries(&trait_sigs);
    let [node_query] = entries.as_slice() else {
        panic!("expected one namespace entry, found {}", entries.len());
    };
    let catalog = node_query.impls().entries();
    assert_eq!(catalog.len(), 1, "one trait impl");
    let SourceImplEntry::TraitImpl(trait_name, signatures) = &catalog[0] else {
        panic!("expected a trait impl, found {:?}", catalog[0]);
    };
    assert_eq!(trait_name.as_str(), "QueryMatcher");
    assert_eq!(
        signatures.len(),
        1,
        "one method signature on the trait impl"
    );
    assert_eq!(signatures[0].name().as_str(), "matches");
    assert_eq!(signatures[0].parameters().len(), 1, "one parameter");
    assert_eq!(signatures[0].parameters()[0].name().as_str(), "candidate");
    assert!(
        matches!(signatures[0].parameters()[0].reference(), SourceReference::Plain(name) if name.as_str() == "Node"),
        "parameter type resolves to Node"
    );
    assert!(
        matches!(signatures[0].return_reference(), SourceReference::Plain(name) if name.as_str() == "Boolean"),
        "return reference resolves to Boolean"
    );
}

/// The macro/engine namespace walk (the second of the two parallel parsers)
/// must accept the same fused and body-optional shapes: a fused entry lowers
/// its inline body to a type declaration while the trailing `{| … |}` block
/// is skipped, and a body-optional `TypeName {| … |}` mints no declaration on
/// this path. This proves the engine walk and the source walk segment entries
/// identically — the boundary the plan flags as the riskiest divergence.
#[test]
fn engine_namespace_walk_accepts_fused_and_body_optional_entries() {
    let source = "[] [] { RecordIdentifier String {| Display Ord |} StatementText {| Display |} Topic String }";
    let schema = SchemaEngine::default()
        .lower_source(source, SchemaIdentity::new("example", "0.1.0"))
        .expect("engine lowers fused and body-optional entries");

    let TypeDeclaration::Newtype(record_identifier) = schema
        .type_named("RecordIdentifier")
        .expect("fused body lowers")
    else {
        panic!("RecordIdentifier should lower to a newtype from its inline body");
    };
    assert_eq!(record_identifier.reference, TypeReference::String);

    let TypeDeclaration::Newtype(topic) = schema
        .type_named("Topic")
        .expect("entry after an impl block still lowers")
    else {
        panic!("Topic should lower to a newtype");
    };
    assert_eq!(topic.reference, TypeReference::String);

    assert!(
        schema.type_named("StatementText").is_none(),
        "a body-optional entry mints no type declaration on the engine path"
    );
}

/// A `{| … |}` impl block must trail a type name — a leading impl block with
/// no preceding head is rejected, proving the entry walk does not silently
/// swallow a stray pipe-brace.
#[test]
fn leading_impl_block_is_rejected() {
    let source = "{}\n[]\n[]\n{\n  {| Display |}\n}";
    let error =
        SchemaSourceArtifact::from_schema_text(source).expect_err("leading impl block is rejected");
    let message = error.to_string();
    assert!(
        message.contains("impl block") && message.contains("trail"),
        "error should name the leading-impl-block boundary, got: {message}"
    );
}

// ---- STEP 3: lowering the catalog to an enumerable manifest ----

/// A fused `RecordIdentifier String {| Display Ord |}` lowers to a newtype
/// declaration whose `impls()` enumerates both marker traits, in order.
#[test]
fn fused_markers_lower_onto_the_declaration() {
    let schema = lower_fixture("fused-markers");
    let declaration = schema
        .namespace()
        .iter()
        .find(|declaration| declaration.name().as_str() == "RecordIdentifier")
        .expect("RecordIdentifier lowers");

    let entries = declaration.impls().entries();
    assert_eq!(
        entries.len(),
        2,
        "two marker impls attach to the declaration"
    );
    assert!(matches!(&entries[0], ImplReference::Marker(name) if name.as_str() == "Display"));
    assert!(matches!(&entries[1], ImplReference::Marker(name) if name.as_str() == "Ord"));

    // The schema-wide manifest names the same target/entries.
    let manifest = schema.referenced_impls();
    assert_eq!(manifest.len(), 2, "manifest enumerates both marker entries");
    assert!(
        manifest
            .iter()
            .all(|reference| reference.target().as_str() == "RecordIdentifier"),
        "every fused entry targets RecordIdentifier"
    );
}

/// A trait + method-signature entry lowers to a `TraitImpl` with resolved
/// parameter and return type references — the catalog carries callable
/// signatures, not opaque atoms.
#[test]
fn trait_method_signatures_lower_with_resolved_references() {
    let schema = lower_fixture("trait-method-sigs");
    let declaration = schema
        .namespace()
        .iter()
        .find(|declaration| declaration.name().as_str() == "NodeQuery")
        .expect("NodeQuery lowers");

    let entries = declaration.impls().entries();
    let [ImplReference::TraitImpl(trait_name, methods)] = entries else {
        panic!("expected one trait impl, found {entries:?}");
    };
    assert_eq!(trait_name.as_str(), "QueryMatcher");
    assert_eq!(methods.len(), 1, "one method signature on the trait impl");
    let signature = &methods[0];
    assert_eq!(signature.name().as_str(), "matches");
    assert_eq!(signature.parameters().len(), 1, "one parameter");
    assert_eq!(signature.parameters()[0].name().as_str(), "candidate");
    assert_eq!(
        signature.parameters()[0].reference(),
        &TypeReference::Plain(Name::new("Node")),
        "parameter type resolves to a Node reference"
    );
    assert_eq!(
        signature.return_reference(),
        &TypeReference::Boolean,
        "return type resolves to the Boolean scalar"
    );
}

/// A body-optional `StatementText {| … |}` mints no type declaration but
/// surfaces as a standalone `ImplBlock` targeting `StatementText`, with its
/// catalog enumerable through the schema-wide manifest.
#[test]
fn body_optional_lowers_to_a_standalone_impl_block() {
    let schema = lower_fixture("body-optional");

    assert!(
        schema
            .namespace()
            .iter()
            .all(|declaration| declaration.name().as_str() != "StatementText"),
        "a body-optional entry mints no type declaration"
    );

    let [block] = schema.impl_blocks() else {
        panic!(
            "expected one standalone impl block, found {:?}",
            schema.impl_blocks()
        );
    };
    assert_eq!(block.target().as_str(), "StatementText");
    let entries = block.catalog().entries();
    assert_eq!(entries.len(), 2, "one marker plus one inherent method");
    assert!(matches!(&entries[0], ImplReference::Marker(name) if name.as_str() == "Display"));
    let ImplReference::InherentMethod(signature) = &entries[1] else {
        panic!("expected an inherent method, found {:?}", entries[1]);
    };
    assert_eq!(signature.name().as_str(), "word_count");
    assert!(signature.parameters().is_empty(), "nullary method");
    assert_eq!(signature.return_reference(), &TypeReference::Integer);

    // The manifest reaches the body-optional block's entries by their target.
    let manifest = schema.referenced_impls();
    assert_eq!(
        manifest.len(),
        2,
        "manifest reaches the body-optional entries"
    );
    assert!(
        manifest
            .iter()
            .all(|reference| reference.target().as_str() == "StatementText"),
        "the body-optional entries target StatementText"
    );
}

// ---- STEP 3: the out-of-band trust-boundary verification ----

/// The "available Rust surface" for the trait-method-sigs fixture: the exact
/// facts a real crate would expose — the `QueryMatcher` trait implemented for
/// `NodeQuery`, and the `matches(candidate: Node) -> Boolean` method present
/// on it. Declared by hand here so the seam is exercised without parsing a
/// real crate.
fn node_query_surface() -> RustSurface {
    RustSurface::new(vec![
        ImplFact::trait_impl(Name::new("NodeQuery"), Name::new("QueryMatcher")),
        ImplFact::method(
            Name::new("NodeQuery"),
            MethodSignature::new(
                Name::new("matches"),
                vec![MethodParameter::new(
                    Name::new("candidate"),
                    TypeReference::Plain(Name::new("Node")),
                )],
                TypeReference::Boolean,
            ),
        ),
    ])
}

/// The trust boundary: when every referenced trait/method signature is
/// present on the declared Rust surface, verification passes. This is the
/// out-of-band catalog check the seam needs — the schema references impls
/// that live on the Rust side, and the boundary confirms they exist.
#[test]
fn present_signatures_pass_verification() {
    let schema = lower_fixture("trait-method-sigs");
    node_query_surface()
        .verify_catalog(&schema)
        .expect("a catalog referencing only present signatures verifies");
}

/// The falsifiable half of the trust boundary: a reference to an ABSENT
/// method signature must FAIL with a typed error naming the exact missing
/// signature. Here the surface knows the `QueryMatcher` trait impl but is
/// missing the `matches` method — the catalog references a method the crate
/// does not provide, and verification rejects it.
#[test]
fn absent_method_signature_fails_verification() {
    let schema = lower_fixture("trait-method-sigs");
    // A surface with the trait impl but WITHOUT the `matches` method.
    let surface = RustSurface::new(vec![ImplFact::trait_impl(
        Name::new("NodeQuery"),
        Name::new("QueryMatcher"),
    )]);

    let error = surface
        .verify_catalog(&schema)
        .expect_err("a reference to an absent method must fail verification");

    let SchemaError::UnverifiedImplReference {
        target,
        kind,
        signature,
    } = &error
    else {
        panic!("expected an UnverifiedImplReference error, got: {error}");
    };
    assert_eq!(target, "NodeQuery");
    assert_eq!(*kind, "method signature");
    assert_eq!(
        signature, "matches",
        "the error names the exact unverified method signature"
    );
}

/// The boundary also rejects a reference to an absent TRAIT impl: a marker
/// entry whose trait the surface does not provide fails, naming the trait.
#[test]
fn absent_trait_impl_fails_verification() {
    let schema = lower_fixture("fused-markers");
    // The surface knows `Display` for `RecordIdentifier` but not `Ord`.
    let surface = RustSurface::new(vec![ImplFact::trait_impl(
        Name::new("RecordIdentifier"),
        Name::new("Display"),
    )]);

    let error = surface
        .verify_catalog(&schema)
        .expect_err("a reference to an absent trait impl must fail verification");

    let SchemaError::UnverifiedImplReference {
        target,
        kind,
        signature,
    } = &error
    else {
        panic!("expected an UnverifiedImplReference error, got: {error}");
    };
    assert_eq!(target, "RecordIdentifier");
    assert_eq!(*kind, "trait impl");
    assert_eq!(signature, "Ord", "the error names the absent trait");
}
