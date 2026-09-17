# Roseflower

Dedicated real-time multiplayer tracking and management microservice for [AyanomiBancho](https://github.com/akikohatsune/ayanomibancho).

## Features
- **Real-Time Multiplayer Tracking**: Live monitoring of osu! lobbies, slots readiness, and beatmap selections.
- **Match Rounds & Scores History**: Stores round completions, winner badges, accuracy, and slot scores.
- **High-Performance Architecture**: Powered by Axum and SQLite with WAL mode.
- **RESTful API**:
  - `GET /` or `GET /multi`: Live multiplayer rooms dashboard.
  - `GET /api/multi/rooms`: JSON list of all active multiplayer rooms.
  - `GET /api/multi/rooms/:id`: Detailed live room and round results.
  - `GET /api/multi/stats`: Active rooms, online players, and match statistics.
  - `POST /api/multi/sync`: Live room synchronization from Bancho.
  - `POST /api/multi/disband`: Room cleanup on lobby disband.
  - `POST /api/multi/finish`: Round completion and match scores storage.
  - `GET /health`: Health check endpoint.
- **Subdomain & Gateway Reverse Proxy Support**: Direct standalone access at `roseflower.<domain>` or via `ayanomi_gateway` reverse proxy.

## Configuration (`config.toml`)
```toml
[server]
name = "AyanomiBancho"
host = "127.0.0.1"
port = 5003

[database]
path = "../ayanomibancho!/data/roseflower.db"
fallback_path = "data/roseflower.db"
```

## Running
```bash
cargo run --release
```
