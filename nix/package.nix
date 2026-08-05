{
  lib,
  rustPlatform,
  cmake,
  git,
  perl,
  openssl,
  pkg-config,
}:
let
  inherit (builtins) readFile;
  cargoToml = builtins.fromTOML (readFile ../Cargo.toml);
  version = cargoToml.package.version;
in
rustPlatform.buildRustPackage {
  pname = "dotstate";
  inherit version;

  src = lib.cleanSource ../.;

  cargoLock = {
    lockFile = ../Cargo.lock;
  };

  doCheck = true;

  nativeBuildInputs = [
    cmake
    git
    perl
    pkg-config
  ];

  buildInputs = [
    openssl
  ];

  meta = with lib; {
    description = "A modern, secure, and user-friendly dotfile manager built with Rust";
    homepage = "https://dotstate.serkan.dev";
    license = licenses.mit;
    maintainers = with maintainers; [ ];
    platforms = platforms.linux;
    mainProgram = "dotstate";
    sourceProvenance = with sourceTypes; [ binaryNativeCode ];
  };
}
