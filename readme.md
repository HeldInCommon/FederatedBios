
# Held in Common — onboarding (Leptos)

Basic build of a client-side Leptos 0.8 app

## Run
```
rustup target add wasm32-unknown-unknown
cargo install trunk --locked 
trunk serve --open
```

## Layout

- `src/check.rs`: the `Check` card and `CheckState` (pending / ok / failed). Presentational only.
- `src/probes.rs`: one async fn per check. `node()` really polls `http://localhost:4321/health`;
  `docker()` and `image()` are stubs because a browser tab can't see Docker.
- `src/onboarding.rs`: wires the three checks, the stepper, and the gated "Set up your profile" button.


## System Diagram 

![System Architecture](images/sys-architecture.png)

## Notes

- The node must return CORS headers allowing the page's origin, or the health check will fail.
- Set `SIMULATE_DOCKER_FAILURE = true` in `probes.rs` to preview the failure states.
- Moving to Tauri: keep the UI as is and replace the bodies of `docker()` / `image()` with
  `invoke` calls to Rust commands that shell out to Docker.

## The node

`node/` is the service the onboarding page talks to on `localhost:4321`: a separate
Rust crate (axum) packaged as the `heldincommon/node` Docker image. It stores the
profile and visibility choice as JSON files in `/data`.

| Route         | Methods   | Body                                        |
|---------------|-----------|---------------------------------------------|
| `/health`     | GET       | `{"status":"ok","version":"0.1.0"}`         |
| `/profile`    | GET, PUT  | the `Draft` shape from `src/profile.rs`     |
| `/visibility` | GET, PUT  | `{"reach":"public" \| "commons" \| "members"}` |

GET returns 404 until something has been saved.

Run it without Docker while developing:
```
cd node && cargo run
```

Build and run the image:
```
docker build -t heldincommon/node:dev node
docker run -d --name hic-node -p 127.0.0.1:4321:4321 -v hic-data:/data heldincommon/node:dev
```

Settings (environment variables): `HIC_ADDR`, `HIC_DATA_DIR`, and
`HIC_ALLOWED_ORIGINS` (comma-separated; defaults to the `trunk serve` origins).
Add an origin there if the UI is served from anywhere else.
