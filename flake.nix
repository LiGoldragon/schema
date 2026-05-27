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
          type == "regular" && pkgs.lib.hasSuffix ".schema" path;
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
          no-btree-canonical = pkgs.runCommand "schema-next-no-btree-canonical" { } ''
            if grep -R "BTreeMap" ${src}/src/asschema.rs; then
              echo "BTreeMap must not be canonical assembled-schema storage" >&2
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
