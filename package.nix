{
  rustPlatform,
  perl,
  pkg-config,
  openssl,
}: let
  cargoToml = builtins.fromTOML (builtins.readFile ./Cargo.toml);
in
rustPlatform.buildRustPackage {
  pname = "grafana-sync";
  version = cargoToml.package.version;

  src = ./.;

  buildInputs = [
    perl
    pkg-config
    openssl
  ];

  cargoDeps = rustPlatform.importCargoLock {
    lockFile = ./Cargo.lock;
    allowBuiltinFetchGit = true;
  };

  doCheck = true;
}
