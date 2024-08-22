{
  description = "Rust-Nix";

  inputs = {
    nixpkgs.url =
      "github:NixOS/nixpkgs/216728b751c07bb6066a2b9e26d7fd700723c338"; # nixos-unstable before https://github.com/NixOS/nixpkgs/commit/cf5e2c2c9adb9ae2db58c75a50453ee7d5d6a699

    flake-parts = {
      url = "github:hercules-ci/flake-parts";
      inputs.nixpkgs-lib.follows = "nixpkgs";
    };
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    crate2nix.url = "github:nix-community/crate2nix";

    # Development

    devshell = {
      url = "github:numtide/devshell";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = inputs@{ flake-parts, ... }:
    flake-parts.lib.mkFlake { inherit inputs; } {
      systems =
        [ "x86_64-linux" "aarch64-linux" "x86_64-darwin" "aarch64-darwin" ];

      imports =
        [ ./nix/rust-overlay/flake-module.nix ./nix/devshell/flake-module.nix ];

      perSystem = { pkgs, ... }:
        let
          name = "teledump";
          cargoNix = pkgs.callPackage ./Cargo.nix { };
        in rec {
          checks = {
            teledump = cargoNix.workspaceMembers.${name}.build.override {
              runTests = true;
            };
          };

          packages = {
            teledump = cargoNix.workspaceMembers.${name}.build;
            default = packages.teledump;
          };
        };
    };
}
