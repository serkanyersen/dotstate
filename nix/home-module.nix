{
  config,
  pkgs,
  lib,
  ...
}:
let
  cfg = config.programs.dotstate;
  tomlFormat = pkgs.formats.toml { };
in
{
  options.programs.dotstate = {
    enable = lib.mkEnableOption "Whether to enable dotstate, a terminal-based dotfile manager.";

    package = lib.mkOption {
      type = lib.types.nullOr lib.types.package;
      default = null;
      description = "The dotstate package to use.";
    };

    settings = lib.mkOption {
      type = with lib.types; nullOr (oneOf [ tomlFormat.type str path ]);
      default = null;
      description = ''
        Configuration for dotstate. Written to {file}`$XDG_CONFIG_HOME/dotstate/config.toml`.
        Can be a Nix attrset (converted to TOML), a raw TOML string, or a path to a `.toml` file.
      '';
      example = lib.literalExpression ''
        {
          repo_mode = "Local";
          repo_path = "~/dotfiles";
        }
      '';
    };
  };

  config = lib.mkIf cfg.enable {
    home.packages = lib.optional (cfg.package != null) cfg.package;

    xdg.configFile = lib.mkIf (cfg.settings != null) {
      "dotstate/config.toml".source =
        let
          rawConfig =
            if lib.isString cfg.settings then
              pkgs.writeText "dotstate-config.toml" cfg.settings
            else if builtins.isPath cfg.settings || lib.isStorePath cfg.settings then
              cfg.settings
            else
              tomlFormat.generate "dotstate-config.toml" cfg.settings;
        in
        rawConfig;
    };
  };

  _class = "homeManager";
}
