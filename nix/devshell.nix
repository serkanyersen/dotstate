{
  pkgs,
  dotstate,
}:
pkgs.mkShell {
  inputsFrom = [ dotstate ];

  nativeBuildInputs = with pkgs; [
    cargo
    clippy
    rust-analyzer
    rustc
    rustfmt
  ];

  shellHook = ''
    echo " dotstate dev-shell | 'cargo build' to build | 'cargo test' to test"
  '';
}
