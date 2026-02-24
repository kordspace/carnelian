# carnelian-adapters

Channel adapters for 🔥 Carnelian OS — Telegram and Discord bot integrations.

## Architecture

```
User ──► Bot API ──► Adapter ──► RateLimiter ──► SpamDetector
                                     │
                        ┌────────────┼────────────┐
                        ▼            ▼            ▼
                  PolicyEngine  SessionManager  EventStream
                        │            │            │
                        └────────────┼────────────┘
                                     ▼
                                  Database
```

## Modules

| Module | Description |
|--------|-------------|
| `types` | Core types: `ChannelType`, `TrustLevel`, `ChannelSession`, `PairingRequest` |
| `rate_limiter` | Per-channel-user rate limiting via `governor` |
| `spam_detector` | Message frequency, duplicate content, and command spam scoring |
| `telegram` | Telegram bot adapter using `teloxide` |
| `discord` | Discord bot adapter using `serenity` |
| `db` | CRUD operations for the `channel_sessions` table |
| `config` | Adapter configuration and bot credential management |
| `testing` | Mock adapters and test utilities |

## Trust Level System

| Trust Level | Capabilities | Rate Limit | Session Expiry |
|-------------|-------------|------------|----------------|
| Untrusted | `channel.message.receive` | 5 msg/min | 24 hours |
| Conversational | `channel.message.receive`, `channel.message.send` | 30 msg/min | 30 days |
| Owner | All conversational + `task.create`, `skill.execute`, `config.read` | 100 msg/min | Never |

## Pairing Flow

1. User sends `/pair` (Telegram) or `!pair` (Discord)
2. Bot generates a UUID pairing token, stores it in `channel_sessions.metadata`
3. User sends `/pair <token>` to confirm
4. Bot verifies the token, upgrades trust level, grants capabilities
5. `ChannelPaired` event emitted to the event stream

## API Endpoints

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/v1/channels` | List all channel sessions (optional `?channel_type=` filter) |
| POST | `/v1/channels` | Create a new channel session |
| GET | `/v1/channels/:id` | Get channel session details |
| PUT | `/v1/channels/:id` | Update trust level or metadata |
| DELETE | `/v1/channels/:id` | Delete channel session |
| POST | `/v1/channels/:id/pair` | Initiate pairing flow |

## Setup

### Telegram

1. Create a bot via [@BotFather](https://t.me/BotFather)
2. Set the `TELEGRAM_BOT_TOKEN` environment variable
3. Enable in config: `adapter_telegram_enabled = true`

### Discord

1. Create a bot at [Discord Developer Portal](https://discord.com/developers/applications)
2. Enable the **Message Content Intent** in Bot settings
3. Set the `DISCORD_BOT_TOKEN` environment variable
4. Enable in config: `adapter_discord_enabled = true`

## Configuration

Environment variables:

| Variable | Description |
|----------|-------------|
| `TELEGRAM_BOT_TOKEN` | Telegram bot token (enables Telegram adapter) |
| `DISCORD_BOT_TOKEN` | Discord bot token (enables Discord adapter) |
| `ADAPTER_SPAM_THRESHOLD` | Spam score threshold, 0.0–1.0 (default: 0.8) |
| `ADAPTER_SPAM_TTL_SECS` | Spam score entry TTL in seconds (default: 3600) |

Or in `machine.toml`:

```toml
adapter_telegram_enabled = true
adapter_discord_enabled = true
adapter_spam_threshold = 0.8
```

## Event Stream Integration

All lifecycle events are emitted as `EventType::Custom(...)`:

- `ChannelConnected` — adapter started
- `ChannelDisconnected` — adapter stopped
- `ChannelMessageReceived` — incoming message processed
- `ChannelMessageSent` — outgoing message delivered
- `ChannelPaired` — pairing completed
- `ChannelUnpaired` — session deleted
- `ChannelRateLimited` — rate limit exceeded
- `ChannelSpamDetected` — spam score above threshold

## Security Considerations

- Bot tokens are stored in the `config_store` table. Future enhancement: encrypt with the existing `encryption.rs` utilities.
- Capability grants use the existing `PolicyEngine` with `subject_type = "channel"`.
- Rate limiting and spam detection provide defense-in-depth against abuse.
- Owner trust level requires explicit pairing confirmation (future: signature verification).

## Database

Uses the existing `channel_sessions` table from migration `00000000000002_phase1_delta.sql`:

```sql
CREATE TABLE channel_sessions (
    session_id      UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    channel_type    TEXT NOT NULL CHECK (channel_type IN ('telegram', 'discord', 'whatsapp', 'slack', 'ui')),
    channel_user_id TEXT NOT NULL,
    trust_level     TEXT NOT NULL DEFAULT 'untrusted' CHECK (trust_level IN ('conversational', 'untrusted', 'owner')),
    identity_id     UUID REFERENCES identities(identity_id) ON DELETE SET NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_seen_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    metadata        JSONB NOT NULL DEFAULT '{}',
    UNIQUE (channel_type, channel_user_id)
);
```
