{
  description = "graphsync flake";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs?ref=nixos-unstable";
  };

  outputs =
    {
      self,
      nixpkgs,
    }:
    let
      mkPkgs =
        system:
        import nixpkgs {
          inherit system;
        };
      mkDevshell =
        system: let pkgs = (mkPkgs "x86_64-linux"); in
          pkgs.mkShell {
            packages = with pkgs; [
              cargo
              rustc
              rustfmt
              pkg-config
              openssl
              perl
            ];
          };
    in
    {
      devShells."x86_64-linux".default = mkDevshell "x86_64-linux";
      devShells."aarch64-darwin".default = mkDevshell "aarch64-darwin";
      packages."x86_64-linux".default = (mkPkgs "x86_64-linux").callPackage ./package.nix { };
      packages."aarch64-darwin".default = (mkPkgs "aarch64-darwin").callPackage ./package.nix { };
    };
}
