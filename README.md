# Load-Balancer

## What is it

Load-balancer written for education.

## Features

- Load-balancing algorithms
    - [x] Round Robin
    - [x] Weighted Round Robin
    - [ ] Dynamic Weighted Round Robin
    - [x] Least connections
    - [ ] PEWMA
    - [ ] Power of 2
- Healthcheck
    - [x] Ban til next healthcheck
    - [ ] Ban for interval
    - [ ] Unban all hosts if too many are banned
- Proxy server
    - [x] Connection pool for upstream
    - [ ] Ban host in case of big amount of errors in an interval
    - [ ] Set `X-Real-Ip` / `X-Forwarded-For`
    - [ ] HTTPS for upstream
- Other
    - [x] Config
    - [ ] Config reload
    - [x] Metrics (Prometheus + Grafana)
    - [ ] API

## How to build & run

### Run with cargo

`RUST_LOG=info,load_balancer=debug cargo run -- -c /path/to/config.toml`

### Release build

`cargo build --release`

### Docker dev environment

Build and run:
```bash
# build binary
cargo build
# run dev environment
cd docker/
docker compose up
# make requests
curl http://localhost:80/echo/helloworld
```

Prometheus web UI available on http://localhost:9090.

Grafana web UI with dashboard available on http://localhost:3000 (admin:admin).
