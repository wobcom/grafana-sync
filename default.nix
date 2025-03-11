{ config, pkgs, lib, ... }:

with lib;
let
  cfg = config.services.graphsync;

  graphsync = pkgs.callPackage ./package.nix {};
in
{
  options = {
    services.graphsync = {
      enable = mkEnableOption "GraphSync";
      configFile = mkOption {
        type = types.path;
      };
    };
  };
  config = {
    users.users."graphsync".isNormalUser = true;

    systemd.services.graphsync = {
      after = [ "network.target" ];
      wantedBy = [ "multi-user.target" ];
      description = "GraphSync Grafana Syncing Service";
      serviceConfig = {
        Type = "simple";
        ExecStart = "${graphsync}/bin/graphsync ${cfg.configFile}";
        Restart = "on-failure";
        RestartSec = 5;
        User = "graphsync";
      };
    };
  };
}
