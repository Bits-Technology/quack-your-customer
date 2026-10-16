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
          echo "  Rust $(rustc -V | cut -d' ' -f2) | cargo $(cargo -V | cut -d' ' -f2) | uv $(uv -V | cut -d' ' -f2)"
          echo ""
          echo "  Quick Start:"
          echo "    cargo test --workspace     Check the checkout (no hardware, no network)"
          echo "    cargo fmt --all            Format"
          echo "    cargo clippy --workspace   Lint"
          echo "    pep                        Agent Pep from the terminal: wake, do roulade, sleep"
          echo "    scripts/duck-sim           The simulator pep wraps (docs/robot/simulation.md)"
          echo "    ssh microduck              Visit Pep: robotctl monitor | configure | update"
          echo ""
          echo "  CONTRIBUTING.md covers building, the board target and releases"
          echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
          echo ""
        '';

        # microduck_rl's PEP 517 backend is `uv_build`, which PyPI ships as a prebuilt binary that
        # NixOS cannot run (no /lib64 loader), so `uv sync` there dies building the project itself
        # after installing everything else. nixpkgs builds the same backend from source: put it on
        # PATH (the shim calls `uv-build` by name) and on PYTHONPATH (the venv imports `uv_build`),
        # and build that one project without isolation so uv uses it instead of fetching PyPI's.
        uvBuild = pkgs.python312Packages.uv-build;
        uvConfig = pkgs.writeText "uv.toml" ''
          no-build-isolation-package = ["mjlab-microduck"]
        '';

        # `pep` is a crate in this workspace, and this is how it reaches a prompt without an install
        # step: `cargo run` from the checkout the shell was entered in, so it is always the code on
        # the branch you are on. The first call compiles it; after that cargo returns at once.
        pep = pkgs.writeShellScriptBin "pep" ''
          exec cargo run --quiet --manifest-path "''${PEP_REPO:?pep: enter the repo nix shell first (direnv allow)}/Cargo.toml" -p pep -- "$@"
        '';

        # What the simulator's binary wheels dlopen and expect the system to provide. NixOS has
        # no /usr/lib, so they are put on LD_LIBRARY_PATH: libstdc++/libgomp for mujoco, torch and
        # onnxruntime (robotd dlopens the venv's libonnxruntime too), and the GL and window-system
        # libraries the MuJoCo viewer's bundled glfw opens by name.
        wheelLibs = with pkgs; [
          stdenv.cc.cc.lib
          zlib
          libglvnd
          libxkbcommon
          wayland
          libx11
          libxcursor
          libxext
          libxfixes
          libxi
          libxinerama
          libxrandr
          libxrender
        ];
      in
      {
        devShells.default = pkgs.mkShell (
          {
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

              # The simulated duck (docs/robot/simulation.md): `scripts/duck-sim` runs the daemons
              # against microduck_rl's MuJoCo body, a uv project pinned to Python 3.12. The
              # interpreter has to be this one — a uv-downloaded build has no loader on NixOS.
              pkgs.uv
              pkgs.python312
              uvBuild

              pep
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

            # uv uses the interpreter above and never fetches one of its own.
            UV_PYTHON_DOWNLOADS = "never";
            UV_PYTHON_PREFERENCE = "only-system";
            # Stands in for ~/.config/uv/uv.toml inside this shell; a project's own [tool.uv] still
            # applies on top. See uvBuild above.
            UV_CONFIG_FILE = uvConfig;
            PYTHONPATH = "${uvBuild}/${pkgs.python312.sitePackages}";

            shellHook = ''
              # Where `scripts/duck-sim` finds the MuJoCo half: a microduck_rl checkout beside this
              # repo, unless DUCK_SIM_RL already says otherwise.
              export PEP_REPO="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
              export DUCK_SIM_RL="''${DUCK_SIM_RL:-$(cd "$PEP_REPO/.." && pwd)/microduck_rl}"
              ${welcomeMessage}
            '';
          }
          // lib.optionalAttrs pkgs.stdenv.isLinux {
            LD_LIBRARY_PATH = lib.makeLibraryPath wheelLibs;
          }
        );
      }
    );
}
