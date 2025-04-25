{
  rustPlatform,
  perl,
  pkg-config,
  openssl,
}:

rustPlatform.buildRustPackage {
  pname = "grafana-sync";
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
  #  mv target/release/grafana-sync $out/bin
  #  runHook postInstall
  #'';
}
