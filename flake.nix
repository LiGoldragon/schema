{
  description = "schema — typed schema-language substrate for Persona signal contracts";

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
        src = pkgs.lib.cleanSourceWith {
          src = ./.;
          filter = path: type:
            let
              pathString = toString path;
              fixtureRoot = "${toString ./.}/tests/fixtures";
              prototypeSchemasRoot = "${toString ./.}/prototype/schemas";
            in
            craneLib.filterCargoSources path type
            || pathString == fixtureRoot
            || pkgs.lib.hasPrefix "${fixtureRoot}/" pathString
            || pathString == prototypeSchemasRoot
            || pkgs.lib.hasPrefix "${prototypeSchemasRoot}/" pathString;
          name = "source";
        };
        cargoArtifacts = craneLib.buildDepsOnly { inherit src; strictDeps = true; };
        commonArguments = {
          inherit src cargoArtifacts;
          strictDeps = true;
        };
      in
      {
        packages.default = craneLib.buildPackage commonArguments;
        checks = {
          build = craneLib.cargoBuild commonArguments;
          test = craneLib.cargoTest commonArguments;
          test-upgrade-rule-macro-variant = craneLib.cargoTest (commonArguments // {
            cargoTestExtraArgs = "--test document upgrade_rule_macro_variant_lowers_into_assembled_upgrade_feature -- --exact";
          });
          doc = craneLib.cargoDoc (commonArguments // {
            RUSTDOCFLAGS = "-D warnings";
          });
          fmt = craneLib.cargoFmt { inherit src; };
          clippy = craneLib.cargoClippy (commonArguments // {
            cargoClippyExtraArgs = "--all-targets -- -D warnings";
          });
        };
        devShells.default = pkgs.mkShell {
          name = "schema";
          packages = [ pkgs.jujutsu pkgs.pkg-config toolchain ];
        };
      });
}
