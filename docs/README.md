# Carnelian OS Documentation

## Overview

Carnelian OS is a local-first AI agent mainframe built in Rust.

## Architecture

- **Core**: Rust orchestrator (Axum, Tokio, SQLx)
- **UI**: Dioxus desktop application
- **Workers**: Node.js, Python, Shell execution environments
- **Database**: PostgreSQL 15+ with pgvector
- **Models**: Local inference via Ollama (DeepSeek R1 7B)

## Getting Started

See [SETUP.md](SETUP.md) for installation and configuration instructions.

## Machine Profiles

- **Thummim**: 2080 Super, 32GB RAM (constrained)
- **Urim**: 2080 Ti, 64GB RAM (high-end)
