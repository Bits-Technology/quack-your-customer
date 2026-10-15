{
  description = "quack-your-customer development environment";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-26.05";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    { nixpkgs, flake-utils, ... }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
        inherit (pkgs) lib;

        # Same shape as the cloud repo's banner: versions, then the commands you reach for first.
        welcomeMessage = ''
          echo ""
          echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
          echo "  🦆 quack-your-customer — Agent Pep, Duck Diligence Officer"
          echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
          echo "  Rust $(rustc -V | cut -d' ' -f2) | cargo $(cargo -V | cut -d' ' -f2)"
          echo ""
          echo "  Quick Start:"
          echo "    cargo test --workspace     Check the checkout (no hardware, no network)"
          echo "    cargo fmt --all            Format"
          echo "    cargo clippy --workspace   Lint"
          echo "    ssh microduck              Visit Pep: robotctl monitor | configure | update"
          echo ""
          echo "  CONTRIBUTING.md covers building, the board target and releases"
          echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
          echo ""
        '';
      in
      {
        devShells.default = pkgs.mkShell {
          packages = [
            # Interactive bash (readline) so a direnv-modified PATH does not break shopt/vi mode.
            pkgs.bashInteractive

            # Rust. CONTRIBUTING.md: floor 1.89, and code must not need anything past 1.94.1,
            # the Yocto image's compiler. nixpkgs stable is newer than that ceiling, same as CI.
            pkgs.cargo
            pkgs.rustc
            pkgs.clippy
            pkgs.rustfmt
            pkgs.rust-analyzer

            pkgs.pkg-config
            pkgs.git
            pkgs.jq
          ]
          ++ lib.optionals pkgs.stdenv.isLinux [
            # The C libraries CI apt-installs: padd binds libudev through gilrs, and mediad's
            # GStreamer pipeline is cfg(target_os = "linux"). A Mac needs none of these.
            pkgs.udev
            pkgs.glib
            pkgs.gst_all_1.gstreamer
            pkgs.gst_all_1.gst-plugins-base
            pkgs.gst_all_1.gst-plugins-bad
          ];

          # nixpkgs' rustc does not ship the standard library sources; rust-analyzer reads this.
          RUST_SRC_PATH = "${pkgs.rustPlatform.rustLibSrc}";

          shellHook = ''
            ${welcomeMessage}
          '';
        };
      }
    );
}
