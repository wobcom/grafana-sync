{ config, pkgs, lib, ... }:

with lib;
let
  cfg = config.services.graphsync;

  graphsync = pkgs.callPackage ./package.nix {};

  instanceSlaveSettings = {
    options = {
      apiToken = mkOption {
        type = types.str;
      };
    };
  };

  instanceMasterSettings = {
    url = mkOption {
      type = types.str;
      description = "Grafana Base URL";
    };
    apiToken = mkOption {
      type = types.str;
      description = "Grafana API Token";
    };
    syncTag = mkOption {
      type = types.str;
      description = "Sync Tag for which boards to sync from the master into the slaves";
    };
  };

  configSettings = {
    instanceMaster = instanceMasterSettings;

    instanceSlaves = mkOption {
      type = types.attrsOf (types.submodule instanceSlaveSettings);
      description = "Slave instances, with their base url as the key";
    };

    syncRateMins = mkOption {
      type = types.int;
      description = "Synchronization rate in minutes.";
    };
  };

  configNix = {
    service = {
      instance_master = {
        url = cfg.configuration.instanceMaster.url;
        api_token = cfg.configuration.instanceMaster.apiToken;
        sync_tag = cfg.configuration.instanceMaster.syncTag;
      };
      instance_slaves = attrsets.mapAttrsToList
      (name: value: {
        url = name;
        api_token = value.apiToken;
      }) cfg.configuration.instanceSlaves;
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
