# Grafana Sync

A lightweight Rust service that keeps tagged Grafana dashboards perfectly in-step across any number of Grafana instances.

## Mission

Running several Grafana servers— for multi-region HA, developer sandboxes, on-prem/lab copies, or blue/green upgrades—inevitably leads to diverging dashboards. Grafana Sync solves that by:
1. N → N replication – every instance is both a source and a target.
2. Tag-based selection – you decide which dashboards travel by adding a single tag.
3. Folder mirroring – it creates missing folders automatically.
4. Conflict resolution – the newest version always wins (optionally destructive if a dashboard was deleted).
5. Fast cycles – default 1-minute intervals; suitable for near-real-time updates.
6. Pure Rust – small binary, high performance, no GC pauses.

How it works (in a nutshell)

```
+-----------+            +-----------+            +-----------+
| Grafana 1 |    <--->   | Grafana 2 |    <--->   | Grafana N |
+-----------+            +-----------+            +-----------+
      ↑                        ↑                        ↑
      |                        |                        |
      +------------------------+----------+-------------+---------→
                                          |
                                   +--------------+
                                   | Grafana Sync |
                                   +--------------+
```

1. Discovery – each cycle, Grafana Sync queries every configured instance for dashboards carrying the sync tag.
2. Index by UID – dashboards are grouped by UID across all instances.
3. Merge – if all copies are byte-identical, nothing happens. Otherwise the most recently updated dashboard becomes the source of truth.
4. Replicate – the source dashboard is imported to every other instance (folder is created if needed).
5. Optional purge – if a dashboard disappeared and the destructive logic decides it is truly deleted, it will be removed everywhere.
6. Folder purge - empty folders will be deleted as well.

## Quick start

1. Build or download

### With Cargo (Rust 1.76+)

`cargo install grafana-sync`

Or grab a pre-built binary from the releases.

2. Create a config file

> [!NOTE]
> An empty, default config file will be created for you the first time you start the program.

*config.yaml*
```yaml
sync_tag: "SyncMe"          # Only dashboards with this tag are considered
sync_rate_mins: 1           # Polling interval

instances:                  # Two or more Grafana servers
  - url: https://grafana.example.de
    api_token: token1       # Needs folder+dashboards RW permissions
  - url: http://localhost:3000
    api_token: token2
```

3. Run

`grafana-sync config.yaml`

You should see log lines such as:

```
[2025-04-24T15:03:12Z INFO ] Loaded 2 instance(s):
[2025-04-24T15:03:12Z INFO ]   - https://grafana.example.de
[2025-04-24T15:03:12Z INFO ]   - http://localhost:3000
[2025-04-24T15:03:12Z INFO ] === sync-cycle #0 (2025-04-24 17:03:12.058072 +02:00) ===
[2025-04-24T15:03:12Z INFO ] https://grafana.example.de: 7 sync dashboards
[2025-04-24T15:03:12Z INFO ] http://localhost:3000: 7 sync dashboards
[2025-04-24T15:03:12Z INFO ] 
[2025-04-24T15:03:12Z INFO ] ...
[2025-04-24T15:03:12Z INFO ] 
[2025-04-24T15:03:12Z INFO ] === finished sync-cycle #0 in 210.51555ms ===
```

Leave it running as a systemd service, Docker container or Kubernetes sidecar.

> [!NOTE]
> It's also built with Nix users in mind and packaged accordingly, provided with a ready-to-use dev shell.

> [!NOTE]
> All heavy I/O is async; CPU work is negligible, so a single-CPU VM handles dozens of instances comfortably.

## Configuration reference

| Key            | Type | Default | Description                                                           |
| -------------- | ---- | ------- | --------------------------------------------------------------------- |
| sync_tag	     | str  | SyncMe  | Tag used to select dashboards for replication.                        |
| instances	     | list | —	      | Grafana endpoints with an API token that has Editor rights or higher. |
| sync_rate_mins | int  | 1	      | How often the full bidirectional sync cycle runs.                     |

## Running in production

```
# /etc/systemd/system/grafana-sync.service
[Unit]
Description=Grafana dashboard synchroniser
After=network-online.target

[Service]
ExecStart=/usr/local/bin/grafana-sync /etc/grafana-sync.yaml
Restart=on-failure
User=grafana-sync

[Install]
WantedBy=multi-user.target
```

Metrics & health: expose Prometheus and /health in a future release.

## Development

```
git clone https://github.com/wobcom/grafana-sync.git
cd grafana-sync
cargo test
cargo run # Of course you need to have a config.yaml or pass one in
```

Key crates: reqwest, tokio, serde, chrono, tracing, log.

We follow the Rust 2021 edition and `cargo clippy --all-targets --all-features -- -D warnings` for CI.

## Contributing

Pull requests are welcome! Please open an issue first to discuss major changes.
For security disclosures, please e-mail soc@wobcom.de.

## Roadmap

- [ ] Stabilize Deletion Feature
- [ ] Unit tests for destructive-delete flow
- [ ] Find a way to turn off dashboard syncing
- [ ] Improve deletion algorithm with dashboard versioning 
- [ ] Prometheus metrics
- [ ] Federation?

## License

Licensed under the MIT license. See LICENSE for details.



*Made with ❤️ in Rust*
