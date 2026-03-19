# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

nilcc is a TEE (Trusted Execution Environment) platform that runs Docker Compose workloads inside AMD SEV-SNP Confidential VMs (CVMs). The system provides hardware-based attestation, dm-verity filesystem integrity, LUKS-encrypted volumes, and automatic TLS via Caddy.

## Build & Test Commands

### Rust (workspace)
- `cargo build` — build all workspace members
- `cargo build -p <crate>` — build a specific crate
- `cargo test` — run all tests
- `cargo test -p <crate>` — test a specific crate
- `cargo test <name>` — run tests matching a name pattern
- `cargo +nightly fmt --all -- --check` — check formatting (CI uses nightly)
- `cargo fmt` — format code
- `cargo clippy -- -D warnings` — lint (CI denies all warnings)

### nilcc-api (TypeScript)
- `cd nilcc-api && pnpm install` — install dependencies
- `pnpm test` — run all tests (vitest)
- `pnpm vitest <file>` — run a specific test file
- `pnpm vitest --grep "pattern"` — run tests matching pattern
- `pnpm exec biome ci` — check formatting and linting
- `pnpm fmt` — format code
- `tsc` — type-check
- Tests require Docker services: `docker-compose up -d` in nilcc-api/ starts PostgreSQL and LocalStack

## Code Style

### Rust
- Edition 2024, stable toolchain (nightly for fmt only)
- Max line width: 120 (`rustfmt.toml`)
- Clippy allows `unwrap`/`expect` in tests (`clippy.toml`)

### TypeScript
- Biome for formatting and linting
- Double quotes, trailing commas everywhere
- No unused imports/variables, no parameter reassignment, enforce const
- pnpm 10.10.0, Node.js 24+

## Architecture

### System Components

**nilcc-agent** (Rust) — Baremetal host daemon managing CVMs. Launches QEMU VMs, tracks resources (CPU/memory/GPU/disk), exposes HTTP API (Axum). Uses SQLite for state persistence.

**cvm-agent** (Rust) — Runs inside each CVM. Bootstraps Docker Compose workloads, monitors container health, provides logs/stats, handles TLS via Caddy, submits blockchain heartbeats.

**nilcc-api** (TypeScript) — Central orchestrator. REST API (Hono) for end-users, multi-tenant accounts, workload lifecycle management, distributes workloads to nilcc-agents. Uses PostgreSQL (TypeORM).

**nilcc-attester** (Rust + Python) — In-VM container generating AMD SEV-SNP and NVIDIA GPU attestation reports.

**nilcc-verifier** (Rust) — CLI tool for validating attestation reports and measurement hashes.

**heartbeat-funder** (Rust) — Blockchain daemon for funding and submitting heartbeat transactions (Alloy/Ethereum).

**nilcc-agent-cli / nilcc-admin-cli** (Rust) — CLI clients for agent and admin operations.

### Shared Crates (crates/)
- `attestation-report` — SEV-SNP attestation data structures
- `attestation-verification` — Certificate chain and report signature verification
- `cvm-agent-models` — API contracts between nilcc-agent and cvm-agent
- `nilcc-agent-models` — API contracts for nilcc-agent HTTP endpoints
- `nilcc-artifacts` — Artifact metadata, VmType enum, downloading

### Code Patterns
- **Layered architecture:** Routes → Services → Repositories → Database
- **Trait-based DI:** Abstract traits (VmClient, CvmAgentClient, WorkloadService) with mock implementations for testing
- **Worker pattern:** Background tokio tasks with graceful shutdown (EventWorker, HeartbeatWorker)
- **Repository pattern:** sqlx (Rust/SQLite) and TypeORM (TS/PostgreSQL) with migrations
- **Test utilities:** `mockall` and `rstest` for Rust; custom Vitest fixtures with test-client in nilcc-api

### Database Migrations
- **nilcc-agent:** SQLite migrations in `nilcc-agent/migrations/`
- **nilcc-api:** TypeORM migrations in `nilcc-api/migrations/`
