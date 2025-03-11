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
        description = ''
          YAML formatted config file in the following format:
          ```yaml
          sync_tag: "SyncMe"
          instances:
            - url: "https://example.com"
              api_token: "3qv5ukv8u95usiojfoj0wevrjmw0bt8w0"
            - url: "https://example2.com"
              api_token: "3qv5ukv8u95usiojfoj0wevrjmw0bt8w0"
          sync_rate_mins: 1
          ```
        '';
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
