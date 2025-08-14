{
  config,
  pkgs,
  lib,
  ...
}:

with lib;
let
  cfg = config.services.grafana-sync;

  grafana-sync = pkgs.callPackage ./package.nix { };
in
{
  options = {
    services.grafana-sync = {
      enable = mkEnableOption "Grafana Sync";
      configFile = mkOption {
        description = "Path to a YAML formatted config file.";
        type = types.path;
      };
    };
  };
  config = lib.mkIf cfg.enable {
    users.users."grafana-sync" = {
      isSystemUser = true;
      group = "grafana-sync";
    };
    users.groups."grafana-sync" = {};

    systemd.services.grafana-sync = {
      after = [ "network.target" ];
      wantedBy = [ "multi-user.target" ];
      description = "Grafana Syncing Service";
      serviceConfig = {
        Type = "simple";
        ExecStart = "${grafana-sync}/bin/grafana-sync ${cfg.configFile}";
        Restart = "on-failure";
        RestartSec = 5;
        User = "grafana-sync";
      };
    };
  };
}
