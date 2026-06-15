//! Generation: a [`ValidatedReferenceGrammar`] *emits* the reference resolver
//! as Rust source.
//!
//! This is the load-bearing proof. The dispatch precedence that today is
//! hand-written as match-arm ordering in schema-next's `from_parenthesis_objects`
//! is here a [`ValidatedReferenceGrammar`] value; `From<&ValidatedReferenceGrammar>`
//! reads the declared order and writes the resolver, arm by arm, in that order.
//!
//! Nothing here interprets the grammar at runtime: the output is Rust *text*.
//! The emitted resolver dispatches over placeholder decode hooks (`todo!()`
//! arms over abstract types) — the v0 boundary proves the *structure and
//! precedence*, not runtime wiring against schema-next's real types.

use crate::grammar::{BuiltinArity, BuiltinHead, ReferenceForm};
use crate::validate::ValidatedReferenceGrammar;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};

/// The emitted reference resolver, carried as a Rust token stream.
///
/// Built from a [`ValidatedReferenceGrammar`]; rendered to source via
/// [`ResolverModule::to_resolver_source`].
#[derive(Clone)]
pub struct ResolverModule {
    tokens: TokenStream,
}

impl ResolverModule {
    /// The emitted resolver as a token stream.
    pub fn tokens(&self) -> &TokenStream {
        &self.tokens
    }

    /// The emitted resolver pretty-printed as Rust source, in one
    /// `prettyplease` pass. Panics only if the emitted tokens are not parseable
    /// Rust — which is a generator bug, not a runtime condition.
    pub fn to_resolver_source(&self) -> String {
        let file = syn::parse2::<syn::File>(self.tokens.clone())
            .expect("emitted resolver tokens parse as a Rust file");
        prettyplease::unparse(&file)
    }
}

impl From<&ValidatedReferenceGrammar> for ResolverModule {
    fn from(grammar: &ValidatedReferenceGrammar) -> Self {
        let builtin_arms = grammar
            .forms()
            .iter()
            .filter_map(BuiltinFormEmit::from_form)
            .map(|emit| emit.arm());

        let reserved_guard = ReservedHeadGuard::from_grammar(grammar).arm();

        // Marker arms are emitted only where the grammar declares them.
        // Validation guarantees the shape `Builtin* DeclaredMacro? Application`,
        // so the registry rung appears iff a DeclaredMacro form is present, and
        // the application catch-all is always the final arm. A grammar that
        // declares no registry rung generates a resolver with none — the
        // emitted resolver never carries a stage the grammar did not declare.
        let has_declared_macro = grammar
            .forms()
            .iter()
            .any(|form| matches!(form, ReferenceForm::DeclaredMacro));
        let declared_macro_arm = if has_declared_macro {
            quote! {
                if Self::is_declared_macro(head) {
                    return ::std::result::Result::Ok(Resolution::DeclaredMacro);
                }
            }
        } else {
            quote! {}
        };
        let declared_macro_fn = if has_declared_macro {
            quote! {
                /// Whether the head names a declared macro. A real resolver
                /// consults the macro registry here; the v0 hook is a stub.
                fn is_declared_macro(_head: ::std::option::Option<&str>) -> bool {
                    todo!("consult the declared-macro registry")
                }
            }
        } else {
            quote! {}
        };

        Self {
            tokens: quote! {
                /// Resolution outcomes the generated resolver produces.
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
                        #(#builtin_arms)*
                        #reserved_guard
                        #declared_macro_arm
                        ::std::result::Result::Ok(Resolution::Application)
                    }

                    #declared_macro_fn
                }
            },
        }
    }
}

/// One emitted `Builtin` arm: a reserved head plus the parenthesis object count
/// it must hold. Carries the data each arm reads, so the emission is a method on
/// the data rather than a free helper.
struct BuiltinFormEmit<'form> {
    head: &'form BuiltinHead,
    arity: &'form BuiltinArity,
}

impl<'form> BuiltinFormEmit<'form> {
    fn from_form(form: &'form ReferenceForm) -> Option<Self> {
        match form {
            ReferenceForm::Builtin(head, arity) => Some(Self { head, arity }),
            ReferenceForm::DeclaredMacro | ReferenceForm::Application => None,
        }
    }

    /// An early-return arm: this head at exactly this object count resolves to
    /// the built-in, before any later form is consulted.
    fn arm(&self) -> TokenStream {
        let head = self.head.as_str();
        let object_count = self.arity.block_object_count();
        quote! {
            if head == ::std::option::Option::Some(#head) && object_count == #object_count {
                return ::std::result::Result::Ok(Resolution::Builtin);
            }
        }
    }
}

/// The reserved-head guard, derived from the whole built-in set: a head that is
/// reserved but arrived with no matching arity arm is an error, never a
/// fall-through to the generic application catch-all.
struct ReservedHeadGuard<'grammar> {
    grammar: &'grammar ValidatedReferenceGrammar,
}

impl<'grammar> ReservedHeadGuard<'grammar> {
    fn from_grammar(grammar: &'grammar ValidatedReferenceGrammar) -> Self {
        Self { grammar }
    }

    fn arm(&self) -> TokenStream {
        let reserved: Vec<&str> = self
            .grammar
            .forms()
            .iter()
            .filter_map(ReferenceForm::builtin_head)
            .map(BuiltinHead::as_str)
            .collect();
        if reserved.is_empty() {
            return TokenStream::new();
        }
        let reserved_constant = format_ident!("RESERVED_BUILTIN_HEADS");
        quote! {
            const #reserved_constant: &[&str] = &[#(#reserved),*];
            if let ::std::option::Option::Some(head) = head
                && #reserved_constant.contains(&head)
            {
                return ::std::result::Result::Err(ResolveError::WrongBuiltinArity);
            }
        }
    }
}
