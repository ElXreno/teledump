{
  description = "Teledump flake";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-23.05";

    rust-overlay.url = "github:oxalica/rust-overlay";
    crate2nix = {
      url = "github:kolloch/crate2nix";
      flake = false;
    };

    flake-compat = {
      url = "github:edolstra/flake-compat";
      flake = false;
    };
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay, crate2nix, ... }:
    let name = "teledump";
    in flake-utils.lib.eachDefaultSystem (system:
      let
        # Imports
        rust-toolchain =
          pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;

        pkgsWithouOverlays = import nixpkgs { inherit system; };
        pkgs = import nixpkgs {
          inherit system;
          overlays = [
            rust-overlay.overlays.default
            (self: super: {
              rustc = rust-toolchain;
              cargo = rust-toolchain;
            })
          ];
        };
        inherit (import "${crate2nix}/tools.nix" { inherit pkgs; })
          generatedCargoNix;

        # Project
        project = pkgs.callPackage (generatedCargoNix {
          inherit name;
          src = ./.;
        }) {
          defaultCrateOverrides = pkgs.defaultCrateOverrides // {
            ${name} = oldAttrs:
              {
                inherit buildInputs nativeBuildInputs;
              } // buildEnvVars;
          };
        };

        buildInputs = with pkgs; [ openssl.dev sqlite ];
        nativeBuildInputs = with pkgs; [ rustc cargo pkg-config ];
        buildEnvVars = {
          PKG_CONFIG_PATH = "${pkgs.openssl.dev}/lib/pkgconfig";
        };
      in rec {
        packages = {
          ${name} = project.workspaceMembers.${name}.build;
          default = packages.${name};
        };

        apps = {
          ${name} = flake-utils.lib.mkApp {
            inherit name;
            drv = packages.${name};
          };
          default = apps.${name};
        };

        checks = { ${name} = packages.${name}.override { runTests = true; }; };

        devShells.default = pkgs.mkShell {
          inherit buildInputs;

          nativeBuildInputs = (with pkgsWithouOverlays; [ sea-orm-cli ])
            ++ (with pkgs; [ rustfmt rust-analyzer lldb ]) ++ nativeBuildInputs;

          shellHook = ''
            # For JetBrains CLion
            # Set standart library path to: .rust-src/rust
            ln -sfT ${rust-toolchain}/lib/rustlib/src ./.rust-src
          '';

          RUST_SRC_PATH = "${rust-toolchain}/lib/rustlib/src/rust/src";
        } // buildEnvVars;
      });
}
