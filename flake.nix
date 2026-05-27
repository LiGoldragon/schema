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
          (craneLib.filterCargoSources path type) || (schemaFilter path type);
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
            grep -R '"StructFields"' ${src}/tests/lowering.rs >/dev/null
            grep -R '"EnumVariants"' ${src}/tests/lowering.rs >/dev/null
            ! grep -R "type_declaration_macro:" ${src}/src/engine.rs
            ! grep -R "surface_macro:" ${src}/src/engine.rs
            ! grep -R "matches_pair" ${src}/src/engine.rs
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
