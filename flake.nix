{
  description = "graphsync flake";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs?ref=nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };

  outputs =
    {
      self,
      nixpkgs,
      rust-overlay,
    }:
    let
      mkPkgs =
        system:
        import nixpkgs {
          inherit system;
          overlays = [ rust-overlay.overlays.default ];
        };
    in
    {
      packages."x86_64-linux".default = (mkPkgs "x86_64-linux").callPackage ./package.nix { };
      packages."aarch64-darwin".default = (mkPkgs "aarch64-darwin").callPackage ./package.nix { };
    };
}
