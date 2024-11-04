{
  stdenv,
}:

# Needed for nightly
with import <nixpkgs>
{
  system = "x86_64-linux";
  overlays = [
    (import (fetchTarball "https://github.com/oxalica/rust-overlay/archive/master.tar.gz"))
  ];
};
let
  rustOverlay = import (builtins.fetchTarball "https://github.com/oxalica/rust-overlay/archive/master.tar.gz");

  rustBin = rust-bin.nightly.latest.minimal;

  rustPlatform = makeRustPlatform {
    cargo = rustBin;
    rustc = rustBin;
  };
in
rustPlatform.buildRustPackage rec {
  pname = "graphsync";
  version = "0.1.0";

  src = ../../../../programming/graphsync;

  nativeBuildInputs = [ perl pkg-config ];
  buildInputs = [ perl openssl ];

  cargoHash = "sha256-BjzDc37DpWA5rwzwnqRGIdB/oQY0pCmyKkFrsnd2/DM=";

  #installPhase = ''
  #  runHook preInstall
  #  mkdir -p $out/bin
  #  mv target/release/graphsync $out/bin
  #  runHook postInstall
  #'';
}
