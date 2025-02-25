{ config, pkgs, lib, ... }:

with lib;
let
  cfg = config.services.graphsync;

  graphsync = pkgs.callPackage ./package.nix {};

  instanceSettings = {
    url = mkOption {
      type = types.str;
      description = "Grafana Base URL";
    };
    apiToken = mkOption {
      type = types.str;
      description = "Grafana API Token";
    };
  };

  configSettings = {
    syncTag = mkOption {
      type = types.str;
      description = "Sync Tag for which boards to sync";
    };

    instances = mkOption {
      type = types.attrsOf (types.submodule instanceSettings);
      description = "instances, with their base url as the key";
    };

    syncRateMins = mkOption {
      type = types.int;
      description = "Synchronization rate in minutes.";
    };
  };

  configNix = {
    service = {
      sync_tag = cfg.configuration.syncTag;
      instances = attrsets.mapAttrsToList
      (name: value: {
        url = name;
        api_token = value.apiToken;
      }) cfg.configuration.instances;
      sync_rate_mins = cfg.configuration.syncRateMins;
    };
  };
  configJSON = builtins.toFile "graphsync-config.json" (generators.toJSON {} configNix);
  # Silly hack to get a nice yaml config file.
  configFile = pkgs.runCommand "graphsync-config.yaml" { preferLocalBuild = true; } ''
    ${pkgs.remarshal}/bin/json2yaml -i ${configJSON} -o $out
  '';
in
{
  options = {
    services.graphsync = {
      enable = mkEnableOption "GraphSync";
      configuration = configSettings;
    };
  };
  config = lib.mkIf cfg.enable {
    systemd.services.graphsync = {
      after = [ "network.target" ];
      wantedBy = [ "multi-user.target" ];
      description = "GraphSync Grafana Syncing Service";
      serviceConfig = {
        Type = "simple";
        ExecStart = "${graphsync}/bin/graphsync ${configFile}";
        Restart = "on-failure";
        RestartSec = 5;
      };
    };
  };
}
