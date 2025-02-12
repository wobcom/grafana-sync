{
  stdenv,
}:

# Needed for nightly
with import <nixpkgs>
{
  overlays = [
    (import (fetchTarball "https://github.com/oxalica/rust-overlay/archive/master.tar.gz"))
  ];
};
let
  rustBin = rust-bin.nightly.latest.minimal;

  rustPlatform = makeRustPlatform {
    cargo = rustBin;
    rustc = rustBin;
  };
in
rustPlatform.buildRustPackage rec {
  pname = "graphsync";
  version = "0.1.0";

  src = ./.;

  buildInputs = [ perl pkg-config openssl ];

  cargoHash = "sha256-95YnpkVw3QHrAPdjurqtEkXbV9on8VjDs+VnqnhaMTI=";

  #installPhase = ''
  #  runHook preInstall
  #  mkdir -p $out/bin
  #  mv target/release/graphsync $out/bin
  #  runHook postInstall
  #'';
}
