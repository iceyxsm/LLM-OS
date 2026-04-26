# Ops Quickstart

This folder contains a minimal local monitoring stack for LLM-OS.

## Includes
- Prometheus scrape config: `ops/prometheus.yml`
- Alert rules: `ops/alerts.yml`
- Grafana dashboard: `ops/grafana-dashboard.json`
- Docker compose stack: `ops/docker-compose.yml`

## Prerequisites
- `llmd` running with metrics on `127.0.0.1:9090`
- `policy-engine` running with metrics on `127.0.0.1:9091`

## Start monitoring stack
```bash
cd ops
docker compose up -d
```

## Access
- Prometheus: `http://localhost:9095`
- Grafana: `http://localhost:3001` (admin/admin)

## Alerts included
- `PolicyBreakerOpen`
- `PolicyUnavailableSpike`
- `PolicyDenySpike`
- `PolicyEngineNoTraffic`
