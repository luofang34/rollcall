{
  description = "rollcall — reconcile a declared fleet against observed reality (Rust CLI + libraries).";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-25.11";
  };

  outputs =
    { self, nixpkgs }:
    let
      # x86_64-linux is where the reconciler timer runs; aarch64-darwin is the
      # operator workstation. Build both; CI and the deploy builder cover the
      # Linux target, the Mac covers darwin.
      systems = [
        "x86_64-linux"
        "aarch64-darwin"
      ];
      forAllSystems = f: nixpkgs.lib.genAttrs systems (system: f nixpkgs.legacyPackages.${system});
    in
    {
      packages = forAllSystems (pkgs: rec {
        rollcall = pkgs.rustPlatform.buildRustPackage {
          pname = "rollcall";
          version = "0.1.0";
          src = ./.;
          cargoLock.lockFile = ./Cargo.lock;
          # Build only the CLI; `-p rollcall` pulls in the workspace library
          # crates it depends on (inventory/status/facts/report/netbox).
          cargoBuildFlags = [
            "-p"
            "rollcall"
          ];
          # Several tests spawn `ping` or bind sockets, which the nix build
          # sandbox forbids; `make ci` (cargo test) is the real gate in CI.
          doCheck = false;
          meta = {
            description = "Reconcile a declared fleet against observed reality";
            mainProgram = "rollcall";
          };
        };
        default = rollcall;
      });

      devShells = forAllSystems (pkgs: {
        default = pkgs.mkShell {
          packages = with pkgs; [
            rustc
            cargo
            clippy
            rustfmt
          ];
        };
      });

      formatter = forAllSystems (pkgs: pkgs.nixfmt-rfc-style);
    };
}
