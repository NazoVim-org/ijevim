{
  description = "ijevim - A minimal Vim-like TUI editor written in Rust";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    {
      self,
      nixpkgs,
      flake-utils,
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
        rustPlatform = pkgs.rustPlatform;
      in
      {
        packages = {
          default = rustPlatform.buildRustPackage {
            pname = "ijevim";
            version = "0.1.0";
            src = ./.;
            cargoLock.lockFile = ./Cargo.lock;

            nativeBuildInputs = with pkgs; [
              pkg-config
              libclang
              llvm
            ];

            LIBCLANG_PATH = "${pkgs.libclang.lib}/lib";

            meta = {
              description = "A minimal Vim-like TUI editor written in Rust";
              homepage = "https://github.com/NazoVim-org/ijevim";
              license = pkgs.lib.licenses.mit;
              mainProgram = "ijevim";
            };
          };
        };

        devShells = {
          default = pkgs.mkShell {
            inputsFrom = [ self.packages.${system}.default ];

            packages = with pkgs; [
              cargo
              rustc
              rustfmt
              clippy
              cargo-audit
              git
              gh
              opencode
            ];

            LIBCLANG_PATH = "${pkgs.libclang.lib}/lib";

            RUST_BACKTRACE = "1";

            shellHook = ''
              # Clean stale mlua-sys object files that break vendored Lua build
              find "$HOME/.cargo/registry/src" -path '*/mlua-sys-*/*.o' -delete 2>/dev/null || true

              echo "╔═══════════════════════════════════════════════╗"
              echo "║             ijevim devShell                   ║"
              echo "╠═══════════════════════════════════════════════╣"
              echo "║ Build:  cargo build                           ║"
              echo "║ Run:    cargo run -- <file>                   ║"
              echo "║ Test:   cargo test                            ║"
              echo "║ Clippy: cargo clippy                          ║"
              echo "║ Audit:  cargo audit                           ║"
              echo "║ Binary: ijevim                                 ║"
              echo "╚═══════════════════════════════════════════════╝"
            '';
          };
        };
      }
    );
}
