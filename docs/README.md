# 🔥 Carnelian OS Documentation

## Overview

🔥 Carnelian OS is a local-first AI agent mainframe built in Rust with capability-based security and event-stream architecture.

## Architecture

- **Core Orchestrator**: Rust (Axum, Tokio, SQLx) — HTTP API, WebSocket events, task scheduling
- **Desktop UI**: Dioxus desktop application
- **Workers**: Node.js, Python, Shell execution environments (JSONL transport protocol)
- **Database**: PostgreSQL 16 with pgvector for vector embeddings
- **Models**: Local-first inference via Ollama (DeepSeek R1)
- **Security**: blake3-based hash-chain ledger, capability grants, deny-by-default policy engine

## Getting Started

See [DEVELOPMENT.md](DEVELOPMENT.md) for installation and configuration instructions.

## Guides

| Guide | Description |
|-------|-------------|
| [DEVELOPMENT.md](DEVELOPMENT.md) | Development setup and workflow |
| [DOCKER.md](DOCKER.md) | Docker environment and troubleshooting |
| [BRAND.md](BRAND.md) | 🔥 Dual theme brand kit (Forge / Night Lab) |
| [LOGGING.md](LOGGING.md) | 🔥 Logging philosophy and conventions |

## Machine Profiles

| Profile | GPU | RAM | Recommended Model |
|---------|-----|-----|-------------------|
| **Thummim** | RTX 2080 Super (8GB VRAM) | 32GB | `deepseek-r1:7b` |
| **Urim** | RTX 2080 Ti (11GB VRAM) | 64GB | `deepseek-r1:32b` |

## Brand Identity

- **🔥 Carnelian OS** — System/runtime identity
- **🦎 Lian** — Agent personality
- **💎 Core** — Architectural foundations

See [BRAND.md](BRAND.md) for the complete brand kit.
