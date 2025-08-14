{
  rustPlatform,
  perl,
}: let
  cargoToml = builtins.fromTOML (builtins.readFile ./Cargo.toml);
in
rustPlatform.buildRustPackage {
  pname = "grafana-sync";
  version = cargoToml.package.version;

  src = ./.;

  nativeBuildInputs = [
    perl
  ];

  cargoDeps = rustPlatform.importCargoLock {
    lockFile = ./Cargo.lock;
    allowBuiltinFetchGit = true;
  };

  doCheck = true;
}
