let
  inherit (builtins) fromJSON readFile;

  lock = fromJSON (readFile ./flake.lock);
  namedNode = lock.nodes.${lock.nodes.root.inputs.nixpkgs}.locked;
  nixpkgs = fetchTarball {
    inherit (namedNode) url;
    sha256 = namedNode.narHash;
  };
in
{
  pkgs ? import nixpkgs { },
}:
let
  inherit (pkgs.lib) mkDefault;

  package = pkgs.callPackage ./nix/package.nix { };
in
{
  homeModule = {
    imports = [ ./nix/home-module.nix ];
    programs.dotstate.package = mkDefault package;
  };

  inherit package;
}
