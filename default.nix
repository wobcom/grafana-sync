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
    users.users."grafana-sync".isNormalUser = true;

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
