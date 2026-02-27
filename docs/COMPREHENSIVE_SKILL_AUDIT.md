# Comprehensive CARNELIAN Skill Audit & Migration Plan

**Generated:** 2026-02-26  
**Audit Type:** Complete skill inventory, quality assessment, and migration roadmap

## Executive Summary

**Current Status:**
- **Node.js Skills:** 326 (gateway-based)
- **WASM Skills:** 194 (Rust-based)
- **Total CARNELIAN:** 520 skills
- **THUMMIM Total:** 651 skills
- **Coverage:** 80% (520/651)
- **Remaining:** 131 skills (20%)

---

## Part 1: CARNELIAN Skills Inventory (520 Total)

### Node.js Skills (326) - Gateway-Based Platform Integrations

#### ✅ Well-Implemented Categories

**AI & ML Services (11 skills)**
- anthropic-claude, cohere-generate, google-palm-generate, huggingface-inference
- elevenlabs-tts, whisper-transcribe, openai-image-gen, replicate-run, stability-ai-generate
- image-analyze, image-generate

**Cloud Platforms (15 skills)**
- aws-s3-upload, aws-s3-list, aws-lambda-invoke
- gcp-storage-upload, azure-blob-upload
- digitalocean-deploy, fly-deploy, heroku-deploy, netlify-deploy, railway-deploy, render-deploy, vercel-deploy
- cloudflare-purge-cache, fastly-purge, terraform-apply

**Communication & Messaging (20 skills)**
- slack-send, slack-create-channel, slack-invite-user, slack-react, slack-file, slack-channel
- discord-send, discord-create-channel
- telegram-send, telegram-notify, telegram-group, telegram-media
- teams-send-message, signal-send, matrix-send, mattermost-send, rocketchat-send, zulip-send
- whatsapp-send, email-send

**Databases (10 skills)**
- postgres-query, mongodb-insert, redis-get, redis-set, elasticsearch-index
- supabase-query, planetscale-query, cockroachdb-query
- database-query, sql-query

**Developer Tools & CI/CD (25 skills)**
- github-create-pr, github-create-issue, github-list-prs, github-pr-review, github-actions-dispatch
- gitlab-create-issue, bitbucket-create-pr
- git-branch, git-commit, git-diff, git-log
- circleci-trigger, jenkins-build, travis-trigger
- docker-exec, docker-logs, docker-stats, docker-compose-up
- kubernetes-deploy, kubernetes-get-pods
- code-format, code-review
- terraform-apply

**File Operations (10 skills)**
- file-read, file-write, file-delete, file-move, file-search, file-upload, file-download
- file-analyzer, s3-upload, s3-download

**Calendar & Scheduling (5 skills)**
- google-calendar-create, apple-calendar-create, outlook-calendar-create, caldav-create-event
- calendar-create, calendar-list

**Task Management (8 skills)**
- asana-create-task, linear-create-issue, jira-create-issue, trello-create-card
- todoist-create-task, things-create-task, omnifocus-create-task
- task-create, task-list, task-schedule

**Note-Taking (5 skills)**
- notion-create-page, notion-query-database, obsidian-create-note, obsidian-search
- evernote-create-note, bear-create-note

**CRM & Sales (10 skills)**
- salesforce-create-lead, hubspot-create-contact, pipedrive-create-deal, zoho-create-lead
- intercom-send-message, drift-send-message, zendesk-create-ticket, freshdesk-create-ticket
- helpscout-create-conversation, pagerduty-create-incident

**E-commerce (8 skills)**
- shopify-create-product, woocommerce-create-product, stripe-create-payment, square-create-payment
- paypal-create-payment, ebay-list-item, ebay-search, etsy-create-listing, amazon-product-search, amazon-search-products

**Analytics & Monitoring (15 skills)**
- google-analytics-track, amplitude-track, mixpanel-track, segment-track, analytics-track
- datadog-metric, newrelic-create-event, sentry-capture, bugsnag-notify, rollbar-log
- loggly-send, splunk-search, prometheus-query, grafana-create-dashboard
- metrics-track, metric-track

**Social Media (12 skills)**
- twitter-post, linkedin-post, facebook-post, instagram-post, mastodon-post
- bluesky-post, reddit-post, hackernews-post, producthunt-post, medium-publish
- tiktok-upload, soundcloud-upload

**Finance & Payments (8 skills)**
- plaid-link, plaid-link-token, mint-transactions, alpha-vantage-quote, kraken-ticker
- coinbase-get-price, yahoo-finance-quote, quickbooks-create-invoice, wave-create-invoice, ynab-create-transaction

**Content Management (10 skills)**
- wordpress-create-post, ghost-create-post, contentful-create-entry, sanity-cms-create, strapi-create
- mailchimp-add-subscriber, sendgrid-send-email
- canva-create-design, figma-export, miro-create-board

**Smart Home & IoT (10 skills)**
- homeassistant-call, philips-hue-control, nest-thermostat, ring-doorbell, ecobee-control
- smartthings-device, alexa-skill-invoke, google-home-broadcast
- tesla-control

**Health & Fitness (7 skills)**
- apple-health-query, fitbit-activity, garmin-activity, strava-activity
- myfitnesspal-diary, whoop-recovery, oura-sleep, withings-measures

**Media & Entertainment (15 skills)**
- plex-control, jellyfin-library, emby-library
- sonarr-series, radarr-movie, lidarr-artist, readarr-book
- prowlarr-indexer, overseerr-request, tautulli-stats, ombi-request
- spotify-create-playlist, youtube-upload, twitch-create-clip, mux-create-asset

**Storage & Cloud (8 skills)**
- dropbox-upload, dropbox-download, gdrive-upload, gdrive-list, onedrive-upload
- cloudinary-upload, imgix-transform, wistia-upload

**Communication Services (5 skills)**
- twilio-send-sms, vonage-sms, gmail-send, gmail-read, outlook-send, apple-mail-send

**Workflow & Automation (20 skills)**
- workflow-start, workflow-execute, workflow-status
- pipeline-execute, pipeline-run, saga-execute
- batch-process, debounce-execute, throttle-execute, retry-execute
- circuit-breaker, rate-limit, ratelimit-check
- lock-acquire, lock-release, semaphore-acquire, semaphore-release
- coordinator-elect, schedule-create
- ifttt-trigger, zapier-trigger

**System & Infrastructure (15 skills)**
- system-info, disk-usage, network-stats, process-list, process-monitor
- health-check, heartbeat-send, performance-measure
- gateway-config, gateway-query, gateway-restart, gateway-update
- nodes-list, session-spawn, session-list, session-history, session-status

**Cache & Queue (10 skills)**
- cache-get, cache-set, cache-invalidate
- queue-push, queue-pop, queue-publish, queue-consume
- pubsub-publish, pubsub-subscribe
- redis-get, redis-set

**Data & Transform (10 skills)**
- json-transform, yaml-parse, markdown-parse
- transform-map, text-search, text-to-speech
- hash-file, secret-encrypt
- pdf-generate

**Browser Automation (6 skills)**
- browser-automation, browser-navigate, browser-click, browser-type, browser-screenshot, browser-pdf

**Memory & Storage (6 skills)**
- memory-read, memory-write, storage-read, storage-write, storage-delete
- backup-create

**Misc Platform Integrations (20 skills)**
- airtable-create-record, auth0-create-user, firebase-auth-create
- alert-create, alert-send, notification-send, message-send
- event-emit, trace-log, log-write
- webhook-send, http-request, web-fetch, web-search
- api-call, auth-generate, auth-verify
- config-read, config-write, env-get, env-list

---

### WASM Skills (194) - Rust Self-Contained

#### ✅ Well-Implemented Categories

**Array Operations (13 skills)**
- array-chunk, array-filter, array-find, array-flatten, array-group
- array-join, array-reduce, array-reverse, array-slice, array-sort
- array-stats, array-sum, array-unique

**Encoding (8 skills)**
- base32-encode, base32-decode, base64-encode, base64-decode
- hex-encode, hex-decode
- url-parse, url-build, url-validate

**Cryptography (6 skills)**
- crypto-encrypt, crypto-decrypt, crypto-hash, crypto-sign, crypto-verify, crypto-keygen
- password-hash, jwt-encode, jwt-decode

**Hashing (3 skills)**
- hash-md5, hash-sha256, hash-file

**String Operations (6 skills)**
- string-split, string-trim, string-pad, string-reverse, string-repeat
- string-case, string-join, slug-generate

**Math Operations (8 skills)**
- math-abs, math-calculate, math-clamp, math-max, math-min
- math-pow, math-round, math-sqrt, number-format, number-random

**Date/Time (2 skills)**
- datetime-format, datetime-parse, duration-parse

**File Operations (6 skills)**
- file-metadata, file-checksum, file-watch, file-delete, file-move, file-search, file-write

**Compression (4 skills)**
- archive-tar, archive-zip, gzip-compress, gzip-decompress

**Data Formats (15 skills)**
- json-parse, json-stringify, json-validate, json-diff, json-filter, json-merge
- yaml-parse, toml-parse, toml-generate, ini-parse
- csv-parse, csv-generate
- xml-parse, xml-generate

**Image Processing (5 skills)**
- image-analyze, image-convert, image-generate, image-metadata, image-resize

**Text Processing (8 skills)**
- text-search, text-similarity, text-truncate, template-render
- regex-match, regex-replace, diff-text
- markdown-parse, html-escape

**Code Analysis (7 skills)**
- code-format, code-lint-js, code-lint-python
- code-ast-js, code-ast-python
- code-deps-js, code-deps-python

**Color Operations (2 skills)**
- color-convert, color-parse

**Network (2 skills)**
- ip-parse, network-stats

**System (8 skills)**
- disk-usage, process-list, system-healthcheck
- env-get, env-parse
- path-join, path-parse

**PDF Operations (2 skills)**
- pdf-extract-text, pdf-metadata

**QR Codes (2 skills)**
- qr-generate, qr-decode

**Validation (3 skills)**
- email-validate, data-validate, schema-generate

**Misc Utilities (20 skills)**
- echo, uuid-generate, units-convert, stats-analyze
- bytes-to-string, patch-apply, chart-generate
- canvas-render, skill-creator
- object-merge, object-pick
- sql-query, sql-schema
- graphql-query
- bundles, manifests

**Browser/Discord/Telegram (15 skills)**
- browser-automation, browser-click, browser-navigate, browser-pdf, browser-screenshot, browser-type
- discord-channel, discord-guild, discord-moderate, discord-role, discord-send
- telegram-group, telegram-media, telegram-send

**Gateway/Session/Agents (15 skills)**
- gateway-config, gateway-query, gateway-restart, gateway-update
- session-history, session-list, session-spawn, session-status
- agents-list, agent-step, cascade-run, nodes-list
- memory-read, memory-write, message-send

**Cron/Scheduling (5 skills)**
- cron-add, cron-list, cron-remove, cron-run, cron-schedule

**Communication (8 skills)**
- email-send, email-parse, email-validate
- slack-channel, slack-file, slack-react, slack-send
- whatsapp-send

**Web/HTTP (3 skills)**
- web-fetch, web-search, http-request, http-webhook

**Git Operations (5 skills)**
- git-branch, git-commit, git-diff, git-log, git-status

**Docker (3 skills)**
- docker-exec, docker-logs, docker-stats

**AI/ML (3 skills)**
- model-usage, openai-image-gen, text-to-speech

**Local Places (1 skill)**
- local-places

---

## Part 2: Quality Assessment - Skills Needing Enhancement

### 🔶 Basic Implementation - Need More Features

**Node.js Skills Needing Enhancement:**

1. **file-* operations** - Basic CRUD, need:
   - Streaming support for large files
   - Batch operations
   - Advanced search with filters
   - Permission management

2. **database-query** - Generic wrapper, need specific:
   - Connection pooling
   - Transaction support
   - Migration tools
   - Query builders

3. **email-send** - Basic sending, need:
   - Template support
   - Attachment handling
   - Bulk sending
   - Tracking/analytics

4. **cache-* operations** - Simple get/set, need:
   - TTL management
   - Cache invalidation patterns
   - Distributed caching
   - Cache warming

5. **workflow-* operations** - Basic execution, need:
   - Conditional branching
   - Error handling/retry
   - State persistence
   - Parallel execution

6. **browser-* automation** - Basic actions, need:
   - Wait strategies
   - Element selection helpers
   - Screenshot comparison
   - Network interception

### 🔶 WASM Skills Needing Native Implementation

**Currently Missing Native Rust Implementations:**

1. **Advanced Text Processing**
   - Fuzzy string matching (Levenshtein distance)
   - Text stemming/lemmatization
   - Language detection
   - Sentiment analysis

2. **Advanced Image Processing**
   - Image cropping/rotation
   - Filters and effects
   - Watermarking
   - EXIF manipulation
   - Thumbnail generation

3. **Audio/Video Processing**
   - Audio metadata extraction
   - Video thumbnail extraction
   - Subtitle parsing (SRT, VTT)
   - Format detection

4. **Advanced Compression**
   - 7-Zip support
   - RAR extraction
   - Brotli compression
   - Zstandard

5. **Network Operations**
   - WebSocket client
   - gRPC client
   - DNS lookup
   - TCP/UDP sockets

6. **Data Formats**
   - Protocol Buffers
   - MessagePack
   - Apache Avro
   - Apache Parquet
   - FlatBuffers

7. **Advanced Crypto**
   - Age encryption
   - PGP operations
   - SSH key generation
   - TLS certificate generation
   - TOTP/2FA generation

---

## Part 3: Missing THUMMIM Skills (131 Remaining)

### High-Priority Missing Skills (60 skills)

**Apple Ecosystem (15 skills)**
- apple-contacts, apple-docs, apple-docs-mcp, apple-mail (full suite), apple-mail-search
- apple-media, apple-music (6 sub-skills), apple-notes, apple-photos (10 sub-skills)
- apple-reminders, apple-remind-me (7 sub-skills)

**Calendar & Time Management (8 skills)**
- calcurse, calctl, caldav-calendar (full implementation)
- meeting-scheduler, timezone-converter, availability-checker, calendar-sync

**Advanced Task Management (10 skills)**
- ticktick (13 sub-skills), basecamp-cli (16 sub-skills)
- omnifocus (9 sub-skills - only basic implemented)
- project-management workflows

**Note-Taking & Knowledge (5 skills)**
- roam-research, logseq, dendron
- apple-notes (native), better-notion

**Cloud Storage (3 skills)**
- icloud-drive, box, r2-upload (10 sub-skills)

**Developer Tools (5 skills)**
- cursor-agent, coding-agent, deploy-agent
- coolify (3 sub-skills), dokploy (6 sub-skills)

**AI & ML Advanced (8 skills)**
- gemini-computer-use, gemini-deep-research, gemini-stt
- assemblyai-transcribe, edge-tts (8 sub-skills)
- elevenlabs-agents, elevenlabs-voices (10 sub-skills)

**Finance & Banking (6 skills)**
- bankr (17 sub-skills), copilot-money
- card-optimizer, expense-tracker-pro

### Medium-Priority Missing Skills (40 skills)

**Smart Home Extended (20 skills)**
- homey (22 sub-skills), anova-oven, dyson-cli (6 sub-skills)
- chromecast, chromecast-control, roku (8 sub-skills)
- apple-tv, samsung-tv, lg-webos
- sonos, nanoleaf, govee-lights (3 sub-skills)
- unifi (10 sub-skills), pihole (3 sub-skills), tailscale (3 sub-skills)

**Health & Fitness Extended (10 skills)**
- fitbit-analytics (10 sub-skills), dexcom (2 sub-skills)
- endurance-coach (10 sub-skills), healthkit-sync (5 sub-skills)
- hevy (9 sub-skills), oura-analytics (34 sub-skills)

**Content & Publishing (5 skills)**
- bearblog (4 sub-skills), bookstack (2 sub-skills)
- substack-formatter (7 sub-skills)

**Research & Learning (5 skills)**
- arxiv-watcher, brave-search (5 sub-skills), exa (4 sub-skills), exa-plus (4 sub-skills)
- deepwiki (2 sub-skills)

### Lower-Priority/Specialized Skills (31 skills)

**Regional/Specialized (15 skills)**
- bahn, swiss-transport, swiss-weather, uk-trains
- checkers-sixty60, irish-takeaway, idealista
- cpc-mpqc-competence-tracker-compliance-uk
- drivers-hours-wtd-infringement-coach-uk
- dvsa-tc-audit-readiness-operator-licence-uk
- incident-pcn-evidence-appeal-corrective-actions-uk
- transport-investigation-acas-aligned-pack

**Niche Tools (16 skills)**
- 1password, bitwarden, dashlane
- bambu-cli, camsnap, charger
- cat-fact, coloring-page, dilbert, office-quotes
- bible, clippy, sudoku (10 sub-skills)
- anti-captcha, blockchain-attestation

---

## Part 4: Recommended Implementation Strategy

### Phase 1: Complete Core Platform Integrations (30 skills)

**Priority: Immediate**

1. **Apple Ecosystem (10 skills)**
   - apple-contacts, apple-docs, apple-mail-search, apple-media, apple-notes
   - apple-music-* (play, pause, next, previous, volume)
   - apple-photos-* (list, search, export, import, album)

2. **Advanced Calendar (5 skills)**
   - calcurse, calctl, meeting-scheduler, timezone-converter, calendar-sync

3. **Task Management (5 skills)**
   - ticktick-*, basecamp-*, omnifocus-* (advanced features)

4. **Knowledge Management (5 skills)**
   - roam-research, logseq, dendron, better-notion, apple-notes

5. **Cloud Storage (3 skills)**
   - icloud-drive, box, r2-upload

6. **AI/ML (2 skills)**
   - gemini-computer-use, assemblyai-transcribe

### Phase 2: Add Native Rust/WASM Skills (40 skills)

**Priority: High Value**

1. **Advanced Text Processing (8 skills)**
   - fuzzy-match, levenshtein-distance, text-stemming, text-tokenization
   - language-detect, sentiment-analysis, text-similarity-advanced

2. **Image Processing (8 skills)**
   - image-crop, image-rotate, image-filter, image-watermark
   - image-optimize, exif-read, exif-write, thumbnail-gen

3. **Audio/Video (6 skills)**
   - audio-metadata, audio-convert, video-metadata, video-thumbnail
   - subtitle-parse, m3u8-parse

4. **Advanced Compression (4 skills)**
   - 7z-ops, rar-extract, brotli-compress, zstd-compress

5. **Network Operations (6 skills)**
   - websocket-client, grpc-client, dns-lookup, tcp-socket, udp-socket

6. **Data Formats (8 skills)**
   - protobuf-encode, protobuf-decode, msgpack-encode, msgpack-decode
   - avro-encode, avro-decode, parquet-read, parquet-write

### Phase 3: Smart Home & IoT (20 skills)

**Priority: Medium**

1. **Media Centers (5 skills)**
   - homey-*, roku-*, chromecast-*, apple-tv-*, samsung-tv-*

2. **Lighting & Climate (5 skills)**
   - nanoleaf-*, sonos-*, govee-*, dyson-*

3. **Network & Security (5 skills)**
   - unifi-*, pihole-*, tailscale-*

4. **Specialized Devices (5 skills)**
   - anova-oven, camsnap, charger

### Phase 4: Health, Finance, Regional (41 skills)

**Priority: Lower**

1. **Health Extended (15 skills)**
   - fitbit-analytics-*, dexcom-*, endurance-coach-*, healthkit-sync-*
   - hevy-*, oura-analytics-*

2. **Finance Extended (10 skills)**
   - bankr-*, copilot-money, card-optimizer, expense-tracker-pro

3. **Regional Services (16 skills)**
   - Transport, compliance, specialized regional tools

---

## Part 5: Implementation Checklist

### Immediate Actions (Next 30 Skills to 85%)

- [ ] apple-contacts
- [ ] apple-docs
- [ ] apple-mail-search
- [ ] apple-media
- [ ] apple-notes
- [ ] apple-music-play
- [ ] apple-music-pause
- [ ] apple-music-next
- [ ] apple-music-volume
- [ ] apple-photos-list
- [ ] calcurse
- [ ] calctl
- [ ] meeting-scheduler
- [ ] timezone-converter
- [ ] calendar-sync
- [ ] ticktick-create
- [ ] ticktick-list
- [ ] ticktick-complete
- [ ] roam-research-create
- [ ] logseq-create
- [ ] dendron-create
- [ ] icloud-drive-upload
- [ ] icloud-drive-list
- [ ] box-upload
- [ ] box-list
- [ ] gemini-computer-use
- [ ] assemblyai-transcribe
- [ ] fuzzy-match
- [ ] levenshtein-distance
- [ ] image-crop

### Quality Improvements Needed

- [ ] Enhance file-* operations with streaming
- [ ] Add database connection pooling
- [ ] Improve email-send with templates
- [ ] Add cache TTL management
- [ ] Enhance workflow conditional logic
- [ ] Improve browser automation wait strategies

---

## Conclusion

**Current Achievement:** 80% coverage (520/651 skills)

**Path to 100%:**
- Phase 1: +30 skills → 85% (550 skills)
- Phase 2: +40 skills → 91% (590 skills)
- Phase 3: +20 skills → 94% (610 skills)
- Phase 4: +41 skills → 100% (651 skills)

**Quality Focus:**
- Enhance existing basic implementations
- Add native Rust/WASM for performance-critical operations
- Ensure comprehensive error handling and validation
- Add proper documentation and examples

**Next Steps:**
1. Implement Phase 1 core platform integrations (30 skills)
2. Add Phase 2 native Rust/WASM skills (40 skills)
3. Continue systematic migration through Phases 3-4
4. Continuously improve quality of existing skills

---

*This audit provides a complete roadmap for achieving 100% THUMMIM skill coverage in CARNELIAN with high-quality, performant implementations.*
