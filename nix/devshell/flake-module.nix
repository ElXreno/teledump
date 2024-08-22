{ inputs, lib, ... }: {
  imports = [ inputs.devshell.flakeModule ];

  config.perSystem = { system, pkgs, pkgsWithoutOverlays, ... }: {
    config.devshells.default = {
      imports = [
        "${inputs.devshell}/extra/language/c.nix"
        # "${inputs.devshell}/extra/language/rust.nix"
      ];

      commands = with pkgs; [
        {
          package = rust-toolchain;
          name = "rustc";
          help = "Rust toolchain v${rust-toolchain.version}";
          category = "rust";
        }
        {
          package = inputs.crate2nix.packages.${system}.default;
          name = "crate2nix";
          category = "rust";
        }
      ];

      packages = with pkgsWithoutOverlays;
        [
          nix-prefetch-git # for crate2nix, required if hydra already installed
        ];

      language.c = {
        libraries = lib.optional pkgs.stdenv.isDarwin pkgs.libiconv;
      };

      # For JetBrains CLion
      # Set standart library path to: .rust-src/rust
      devshell.startup.jetbrains-fix.text = ''
        ln -sfT ${pkgs.rust-toolchain}/lib/rustlib/src ./.rust-src
      '';
    };
  };
}
