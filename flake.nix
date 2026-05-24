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
          filter = craneLib.filterCargoSources;
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
