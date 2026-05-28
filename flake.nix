{
  description = "schema-next — position-aware schema engine and assembled schema";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    crane.url = "github:ipetkov/crane";
  };

  outputs = { self, nixpkgs, flake-utils, fenix, crane }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };
        toolchain = fenix.packages.${system}.stable.withComponents [
          "cargo"
          "rustc"
          "rustfmt"
          "clippy"
          "rust-src"
        ];
        craneLib = (crane.mkLib pkgs).overrideToolchain toolchain;
        schemaFilter = path: type:
          type == "regular" && (
            pkgs.lib.hasSuffix ".schema" path
            || pkgs.lib.hasSuffix ".witness.txt" path
          );
        sourceFilter = path: type:
          type == "directory" || (craneLib.filterCargoSources path type) || (schemaFilter path type);
        src = pkgs.lib.cleanSourceWith {
          src = ./.;
          filter = sourceFilter;
          name = "source";
        };
        cargoVendorDirectory = craneLib.vendorCargoDeps { inherit src; };
        commonArguments = {
          inherit src cargoVendorDirectory;
          strictDeps = true;
        };
        cargoArtifacts = craneLib.buildDepsOnly commonArguments;
      in
      {
        packages.default = craneLib.buildPackage (commonArguments // { inherit cargoArtifacts; });
        checks = {
          build = craneLib.cargoBuild (commonArguments // { inherit cargoArtifacts; });
          test = craneLib.cargoTest (commonArguments // { inherit cargoArtifacts; });
          design-examples = pkgs.runCommand "schema-next-design-examples" { } ''
            grep -R "design_example_schema_document_has_three_roots_or_four_with_imports" ${src}/tests/design_examples.rs >/dev/null
            grep -R "design_example_namespace_brace_is_pair_style_key_value_map" ${src}/tests/design_examples.rs >/dev/null
            grep -R "design_example_macro_captures_use_dollar_and_dollar_star_sigils" ${src}/tests/design_examples.rs >/dev/null
            grep -R "design_example_colon_qualified_name_decomposes_into_segments" ${src}/tests/design_examples.rs >/dev/null
            grep -R "design_example_default_engine_has_two_macro_layers" ${src}/tests/design_examples.rs >/dev/null
            grep -R "design_example_schema_lowering_records_source_structure_header" ${src}/tests/design_examples.rs >/dev/null
            grep -R "design_example_macro_node_definitions_separate_structural_from_tagged_invocation" ${src}/tests/design_examples.rs >/dev/null
            grep -R "design_example_schema_node_macro_call_is_tagged_data" ${src}/tests/design_examples.rs >/dev/null
            grep -R "design_example_user_declared_macros_extend_structural_and_named_slots" ${src}/tests/design_examples.rs >/dev/null
            grep -R "design_example_root_enum_uses_direct_variant_shapes" ${src}/tests/design_examples.rs >/dev/null
            grep -R "design_example_same_name_payload_variant_uses_explicit_payload" ${src}/tests/design_examples.rs >/dev/null
            grep -R "design_example_signal_nexus_and_sema_are_schema_declared_planes" ${src}/tests/design_examples.rs >/dev/null
            touch $out
          '';
          no-nested-root-enum-examples = pkgs.runCommand "schema-next-no-nested-root-enum-examples" { } ''
            if grep -R -n -E '^\s*\((Input|Output) \(' ${src}/schemas ${src}/tests/fixtures; then
              echo "schema examples must not reintroduce labeled Input/Output root enums" >&2
              exit 1
            fi
            if grep -R -n -E '@(Vec|Option|KeyValue|Bag|HashSet)' ${src}/schemas ${src}/tests ${src}/src; then
              echo "schema examples must not reintroduce the old @ macro sigil" >&2
              exit 1
            fi
            if grep -R -n -E '\[\[[A-Z]|\((records|kinds|services|Listed) \[[A-Z]|\((byTopic|Projected|nodes) \{[A-Z]' ${src}/schemas ${src}/tests/fixtures; then
              echo "schema examples must use typed NOTA composite references: (Vec T), (Map (K V)), (Optional T)" >&2
              exit 1
            fi
            if grep -R -n -E '\((Vec|Option|KeyValue|Map) \[' ${src}/schemas ${src}/tests; then
              echo "schema examples must not put raw vectors inside composite type constructors" >&2
              exit 1
            fi
            if grep -R -n -E '[A-Za-z][A-Za-z0-9]*\*' ${src}/tests/fixtures ${src}/schemas/spirit-min.schema; then
              echo "schema examples must not reintroduce star-suffix same-name payload sugar" >&2
              exit 1
            fi
            if grep -R -n -E 'SchemaEnumDefinitionBrace|BraceEnum|ExpectedEvenBraceEnumPairs' ${src}/src ${src}/schemas ${src}/tests; then
              echo "brace enum sugar must not reappear; braces are key/value maps" >&2
              exit 1
            fi
            touch $out
          '';
          no-btree-canonical = pkgs.runCommand "schema-next-no-btree-canonical" { } ''
            if grep -R "BTreeMap" ${src}/src/asschema.rs; then
              echo "BTreeMap must not be canonical assembled-schema storage" >&2
              exit 1
            fi
            touch $out
          '';
          no-obsolete-asschema-syntax = pkgs.runCommand "schema-next-no-obsolete-asschema-syntax" { } ''
            if find ${src} -name '*.asschema' -print -quit | grep .; then
              echo "obsolete .asschema syntax fixtures must not remain in schema-next" >&2
              exit 1
            fi
            grep -R "asschema_data_model_is_built_from_real_schema_fixture" ${src}/tests/asschema_definition.rs >/dev/null
            grep -R "raw_core_schema_fixture_is_legal_nota_before_schema_reading" ${src}/tests/raw_core_schema.rs >/dev/null
            if grep -R -n -E '\[Input \[|\[Output \[|\(Struct \[|\(Enum \[|\(Newtype \[|\(Map \[\(Plain|\(Carries \(Plain' ${src}/src ${src}/tests ${src}/schemas; then
              echo "obsolete ASSchema vector-record syntax must not remain in active code or fixtures" >&2
              exit 1
            fi
            touch $out
          '';
          no-authored-features = pkgs.runCommand "schema-next-no-authored-features" { } ''
            if grep -R "EffectTable\\|FanOutTargets\\|StorageDescriptor\\|Features" ${src}; then
              echo "retracted authored schema features are forbidden" >&2
              exit 1
            fi
            touch $out
          '';
          macro-registry-used = pkgs.runCommand "schema-next-macro-registry-used" { } ''
            grep -R "pub struct MacroRegistry" ${src}/src/macros.rs >/dev/null
            grep -R "SchemaEngine::with_registry" ${src}/tests/lowering.rs >/dev/null
            grep -R "lower_source_with_context" ${src}/tests/lowering.rs >/dev/null
            grep -R "default_engine_dispatches_through_registered_macros" ${src}/tests/lowering.rs >/dev/null
            grep -R '"SchemaStructFields"' ${src}/tests/lowering.rs >/dev/null
            grep -R '"SchemaEnumVariants"' ${src}/tests/lowering.rs >/dev/null
            ! grep -R "type_declaration_macro:" ${src}/src/engine.rs
            ! grep -R "surface_macro:" ${src}/src/engine.rs
            ! grep -R "matches_pair" ${src}/src/engine.rs
            touch $out
          '';
          declarative-schema-macros = pkgs.runCommand "schema-next-declarative-schema-macros" { } ''
            grep -R "DeclarativeMacroLibrary::builtin" ${src}/src/engine.rs >/dev/null
            grep -R "SchemaStructDefinition" ${src}/schemas/builtin-macros.schema >/dev/null
            grep -R '\$Name' ${src}/schemas/builtin-macros.schema >/dev/null
            grep -R '\$\*Fields' ${src}/schemas/builtin-macros.schema >/dev/null
            grep -R "expanded_templates" ${src}/tests/lowering.rs >/dev/null
            ! grep -R "struct TypeDeclarationMacro" ${src}/src
            ! grep -R "struct StructFieldsMacro" ${src}/src
            ! grep -R "struct EnumVariantsMacro" ${src}/src
            touch $out
          '';
          namespace-braces-are-key-value = pkgs.runCommand "schema-next-namespace-braces-are-key-value" { } ''
            grep -R "brace_namespace_rejects_parenthesized_named_objects" ${src}/tests/lowering.rs >/dev/null
            grep -R "brace_namespace_rejects_parenthesized_named_objects_even_when_count_is_even" ${src}/tests/lowering.rs >/dev/null
            ! grep -R "NamedTypeDefinition" ${src}/src ${src}/schemas ${src}/tests
            ! grep -R -n -E '^  \([A-Z][A-Za-z0-9]* [\[\(]' ${src}/schemas/root.schema ${src}/schemas/core.schema ${src}/schemas/spirit-min.schema
            touch $out
          '';
          schema-module-entrypoint = pkgs.runCommand "schema-next-schema-module-entrypoint" { } ''
            grep -R "pub struct SchemaPackage" ${src}/src/module.rs >/dev/null
            grep -R "lib.schema" ${src}/src/module.rs >/dev/null
            grep -R "package_loader_reads_schema_lib_entrypoint" ${src}/tests/lowering.rs >/dev/null
            test -f ${src}/tests/fixtures/spirit-crate/schema/lib.schema
            grep -R "colon_qualified_names_lower_as_schema_names" ${src}/tests/lowering.rs >/dev/null
            touch $out
          '';
          raw-core-schema-example = pkgs.runCommand "schema-next-raw-core-schema-example" { } ''
            test -f ${src}/tests/fixtures/raw-core/core.schema
            test -f ${src}/tests/fixtures/raw-core/non-map-root.schema
            test -f ${src}/tests/fixtures/raw-core/odd-map.schema
            grep -R "RawSchemaFile::from_path_and_source" ${src}/tests/raw_core_schema.rs >/dev/null
            grep -R "raw_core_schema_fixture_is_legal_nota_before_schema_reading" ${src}/tests/raw_core_schema.rs >/dev/null
            grep -R "raw_core_schema_file_root_name_comes_from_filename" ${src}/tests/raw_core_schema.rs >/dev/null
            grep -R "raw_core_schema_reads_datatype_key_value_map" ${src}/tests/raw_core_schema.rs >/dev/null
            grep -R "raw_core_schema_preserves_native_key_value_and_pipe_forms" ${src}/tests/raw_core_schema.rs >/dev/null
            grep -R "RawDatatypeMap" ${src}/tests/fixtures/raw-core/core.schema >/dev/null
            grep -F "{ key Name value RawDatatype }" ${src}/tests/fixtures/raw-core/core.schema >/dev/null
            touch $out
          '';
          no-production-free-functions = pkgs.runCommand "schema-next-no-production-free-functions" { } ''
            if grep -R -n -E '^(pub(\([^)]*\))? )?fn ' ${src}/src; then
              echo "production Rust must not use module-level free functions" >&2
              exit 1
            fi
            touch $out
          '';
          no-production-unit-structs = pkgs.runCommand "schema-next-no-production-unit-structs" { } ''
            if grep -R -n -E '^struct [A-Za-z][A-Za-z0-9_]*;' ${src}/src; then
              echo "production Rust must not use unit structs as namespace/method holders" >&2
              exit 1
            fi
            touch $out
          '';
          doc = craneLib.cargoDoc (commonArguments // {
            inherit cargoArtifacts;
            RUSTDOCFLAGS = "-D warnings";
          });
          fmt = craneLib.cargoFmt { inherit src; };
          clippy = craneLib.cargoClippy (commonArguments // {
            inherit cargoArtifacts;
            cargoClippyExtraArgs = "--all-targets -- -D warnings";
          });
        };
        devShells.default = pkgs.mkShell {
          name = "schema-next";
          packages = [ pkgs.jujutsu pkgs.pkg-config toolchain ];
        };
      });
}
