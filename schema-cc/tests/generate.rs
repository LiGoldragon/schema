//! Generation: a validated grammar emits the resolver, in declared precedence.

use nota_next::StructuralMacroNode;
use schema_cc::{ReferenceGrammar, ResolverModule, ValidatedReferenceGrammar};

const CANONICAL: &str = "(ReferenceGrammar (Builtin Vector 1) (Builtin Optional 1) \
                         (Builtin ScopeOf 1) (Builtin Map 2) (Builtin Bytes Atom) \
                         DeclaredMacro Application)";

fn emit(nota: &str) -> ResolverModule {
    let grammar = ReferenceGrammar::from_structural_nota(nota).expect("grammar decodes");
    let validated = ValidatedReferenceGrammar::try_from(grammar).expect("grammar validates");
    ResolverModule::from(&validated)
}

#[test]
fn emitted_tokens_parse_as_valid_rust() {
    let module = emit(CANONICAL);
    syn::parse2::<syn::File>(module.tokens().clone()).expect("emitted resolver is valid Rust");
}

#[test]
fn emitted_resolver_keeps_builtin_arms_in_declared_order() {
    let source = emit(CANONICAL).to_resolver_source();

    let vector = source.find("\"Vector\"").expect("Vector arm present");
    let optional = source.find("\"Optional\"").expect("Optional arm present");
    let scope_of = source.find("\"ScopeOf\"").expect("ScopeOf arm present");
    let map = source.find("\"Map\"").expect("Map arm present");
    let bytes = source.find("\"Bytes\"").expect("Bytes arm present");

    assert!(
        vector < optional && optional < scope_of && scope_of < map && map < bytes,
        "built-in arms must appear in the grammar's declared order:\n{source}"
    );
}

#[test]
fn emitted_resolver_orders_guard_then_macro_then_application_tail() {
    let source = emit(CANONICAL).to_resolver_source();

    let last_builtin = source.find("\"Bytes\"").expect("last built-in arm present");
    let reserved_guard = source
        .find("RESERVED_BUILTIN_HEADS")
        .expect("reserved-head guard present");
    let declared_macro = source
        .find("Resolution::DeclaredMacro")
        .expect("declared-macro fallback present");
    let application = source
        .find("Resolution::Application")
        .expect("application catch-all present");

    assert!(
        last_builtin < reserved_guard,
        "the reserved-head guard follows every built-in arm:\n{source}"
    );
    assert!(
        reserved_guard < declared_macro,
        "the declared-macro fallback follows the reserved-head guard:\n{source}"
    );
    assert!(
        declared_macro < application,
        "the application catch-all is the final tail:\n{source}"
    );
}

#[test]
fn reserved_guard_lists_every_builtin_head() {
    let source = emit(CANONICAL).to_resolver_source();
    // The guard is derived from the Builtin set, not hand-listed.
    for head in ["Vector", "Optional", "ScopeOf", "Map", "Bytes"] {
        let needle = format!("\"{head}\"");
        let occurrences = source.matches(&needle).count();
        assert!(
            occurrences >= 2,
            "{head} appears in both its arm and the reserved-head set; found {occurrences}:\n{source}"
        );
    }
}

#[test]
fn emitted_resolver_matches_golden_source() {
    let source = emit(CANONICAL).to_resolver_source();
    assert_eq!(
        source, GOLDEN,
        "emitted resolver drifted from the golden source:\n{source}"
    );
}

#[test]
fn grammar_without_a_registry_rung_emits_no_macro_stage() {
    // A grammar that declares no DeclaredMacro must generate a resolver with no
    // registry rung — the emitted resolver never carries a stage the grammar
    // did not declare. (The Application catch-all is still its final arm.)
    let source = emit("(ReferenceGrammar (Builtin Vector 1) Application)").to_resolver_source();
    assert!(
        source.contains("\"Vector\""),
        "the declared built-in arm is present:\n{source}"
    );
    assert!(
        !source.contains("is_declared_macro"),
        "no declared-macro rung when the grammar declares none:\n{source}"
    );
    assert!(
        source.contains("Resolution::Application"),
        "the application catch-all is still emitted:\n{source}"
    );
}

const GOLDEN: &str = r#"/// Resolution outcomes the generated resolver produces.
///
/// Emitted by schema-cc from a ReferenceGrammar; the variants
/// stand in for schema-next's real resolution targets at the
/// v0 boundary.
pub enum Resolution {
    /// A reserved built-in head resolved to its built-in type.
    Builtin,
    /// A declared macro, resolved through the registry.
    DeclaredMacro,
    /// A generic application `(Foo A B…)`.
    Application,
}
/// Errors the generated resolver can return.
pub enum ResolveError {
    /// A reserved built-in head appeared with the wrong arity.
    WrongBuiltinArity,
}
/// The generated reference resolver. Its `resolve` method
/// dispatches a parenthesis block in the precedence order the
/// grammar declared.
pub struct ReferenceResolver;
impl ReferenceResolver {
    /// Resolve one parenthesis-reference block. Arms run in the
    /// grammar's declared precedence: each built-in head first,
    /// then a reserved-head arity guard, then the declared-macro
    /// and generic-application fallback tail.
    pub fn resolve(
        &self,
        block: &::nota_next::Block,
    ) -> ::std::result::Result<Resolution, ResolveError> {
        let head = block
            .root_object_at(0)
            .and_then(::nota_next::Block::demote_to_string);
        let object_count = block.holds_root_objects();
        if head == ::std::option::Option::Some("Vector") && object_count == 2usize {
            return ::std::result::Result::Ok(Resolution::Builtin);
        }
        if head == ::std::option::Option::Some("Optional") && object_count == 2usize {
            return ::std::result::Result::Ok(Resolution::Builtin);
        }
        if head == ::std::option::Option::Some("ScopeOf") && object_count == 2usize {
            return ::std::result::Result::Ok(Resolution::Builtin);
        }
        if head == ::std::option::Option::Some("Map") && object_count == 3usize {
            return ::std::result::Result::Ok(Resolution::Builtin);
        }
        if head == ::std::option::Option::Some("Bytes") && object_count == 2usize {
            return ::std::result::Result::Ok(Resolution::Builtin);
        }
        const RESERVED_BUILTIN_HEADS: &[&str] = &[
            "Vector",
            "Optional",
            "ScopeOf",
            "Map",
            "Bytes",
        ];
        if let ::std::option::Option::Some(head) = head
            && RESERVED_BUILTIN_HEADS.contains(&head)
        {
            return ::std::result::Result::Err(ResolveError::WrongBuiltinArity);
        }
        if Self::is_declared_macro(head) {
            return ::std::result::Result::Ok(Resolution::DeclaredMacro);
        }
        ::std::result::Result::Ok(Resolution::Application)
    }
    /// Whether the head names a declared macro. A real resolver
    /// consults the macro registry here; the v0 hook is a stub.
    fn is_declared_macro(_head: ::std::option::Option<&str>) -> bool {
        todo!("consult the declared-macro registry")
    }
}
"#;
