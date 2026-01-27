# Grafana Sync

Grafana Sync keeps tagged dashboards the same across multiple Grafana instances.

## What it does

- Finds dashboards that have a specific tag.
- Treats every instance as both a source and a target.
- Picks the most recently updated dashboard as the winner.
- Imports that dashboard into the other instances.
- Creates missing folders and removes empty folders.

## Quick start

1. Build it.

```bash
cargo install --path .
```

2. Create a config file.

On first run, Grafana Sync will create a default config at the path you pass in.

Example `config.yaml`:

```yaml
sync_tag: "SyncMe"
sync_rate_mins: 1

instances:
  - url: https://grafana.example.com
    api_token: token1
  - url: http://localhost:3000
    api_token: token2
```

The API token should be able to read and write dashboards and folders.

3. Run it.

```bash
grafana-sync config.yaml
```

## How a sync cycle works

Each cycle:

- Query every instance for dashboards with `sync_tag`.
- Group dashboards by UID.
- If they differ, choose the newest one.
- Import the newest one everywhere else.
- Clean up empty folders.

Note: dashboard deletion is not enabled yet. (Experimental)

## Development

```bash
cargo test
cargo run -- config.yaml
```

Before you commit:
```bash
cargo clippy
cargo fmt
cargo test
```

## License

Licensed under MIT. See `LICENSE`.
