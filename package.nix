{
  rust-bin,
  makeRustPlatform,
  perl,
  pkg-config,
  openssl,
}:

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

  buildInputs = [
    perl
    pkg-config
    openssl
  ];

  useFetchCargoVendor = true;
  cargoHash = "sha256-DJPQmcP6i+6JoPA9vOSICTq6yXpYWVG9WPI58fVJVoc=";

  #installPhase = ''
  #  runHook preInstall
  #  mkdir -p $out/bin
  #  mv target/release/graphsync $out/bin
  #  runHook postInstall
  #'';
}
