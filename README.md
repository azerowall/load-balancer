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
    - [ ] Metrics
    - [ ] API

## How to build

`cargo build --release`

## How to run

`RUST_LOG=info,load_balancer=debug cargo run --release`
