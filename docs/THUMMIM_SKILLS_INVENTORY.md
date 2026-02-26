# THUMMIM Skills Inventory vs CARNELIAN Implementation Status

**Generated:** 2026-02-26  
**THUMMIM Skills Found:** 600+ skill directories  
**CARNELIAN Skills Implemented:** 240 skills (40% of 600-skill target)

## Summary Statistics

| Category | THUMMIM Count | CARNELIAN Implemented | Remaining |
|----------|---------------|----------------------|-----------|
| **Total Skills** | 600+ | 240 | 360+ |
| **WASM Skills** | N/A | 86 built + 19 source | - |
| **Node.js Skills** | N/A | 95 | - |
| **Native Ops** | N/A | 26 | - |
| **Python Skills** | N/A | 2 | - |

## CARNELIAN Skills Already Implemented

### WASM Skills (86 built + 19 source ready = 105 total)
- agent-step, agents-list, archive-tar, archive-zip
- array-chunk, array-filter, array-find, array-flatten, array-group, array-join, array-reduce, array-reverse, array-slice, array-sort, array-stats, array-sum, array-unique
- base32-decode, base32-encode, base64-decode, base64-encode
- browser-automation, browser-click, browser-navigate, browser-pdf, browser-screenshot, browser-type
- bytes-to-string, canvas-render, cascade-run, chart-generate
- code-ast-js, code-ast-python, code-deps-js, code-deps-python, code-format, code-lint-js, code-lint-python
- color-convert, color-parse
- cron-add, cron-list, cron-remove, cron-run, cron-schedule
- crypto-decrypt, crypto-encrypt, crypto-hash, crypto-keygen, crypto-sign, crypto-verify
- csv-generate, csv-parse
- datetime-format, datetime-parse
- diff-text, discord-channel, discord-guild, discord-moderate, discord-role, discord-send
- email-parse, email-send, email-validate
- env-get, env-parse
- file-checksum, file-hash, file-metadata, file-watch
- gateway-config, gateway-query, gateway-restart, gateway-update
- git-branch, git-commit, git-diff, git-log, git-status
- hash-file, hash-md5, hash-sha256, html-escape, html-unescape
- http-request, http-webhook
- image-analyze, image-convert, image-generate, image-metadata, image-resize
- ini-parse, json-diff, json-parse, json-stringify, json-transform, json-validate
- jwt-decode, jwt-encode
- markdown-parse, math-abs, math-calculate, math-clamp, math-max, math-min, math-pow, math-round, math-sqrt
- memory-read, memory-write, message-send
- network-stats, nodes-list
- object-merge, object-pick
- password-hash, patch-apply, pdf-extract-text, pdf-metadata, process-list
- qr-decode, qr-generate
- regex-match, regex-replace
- session-history, session-list, session-spawn, session-status
- slack-channel, slack-file, slack-react, slack-send
- slug-generate, sql-query, sql-schema
- string-pad, string-split, string-trim
- system-healthcheck
- telegram-group, telegram-media, telegram-send
- template-render, text-search, text-to-speech
- toml-generate, toml-parse
- units-convert, url-build, url-parse, url-validate
- uuid-generate
- web-fetch, web-search
- whatsapp-send
- xml-generate, xml-parse
- yaml-parse

### Node.js Skills (95 implemented - all use gateway)
- agent-step, agents-list, alert-create, alert-send, api-call
- auth-generate, auth-verify, backup-create, batch-process
- browser-automation, browser-click, browser-navigate, browser-pdf, browser-screenshot, browser-type
- cache-get, cache-invalidate, cache-set, canvas-render, cascade-run
- circuit-breaker, code-format, code-review, config-read, config-write
- coordinator-elect, cron-add, cron-list, cron-remove, cron-run, cron-schedule
- database-query, debounce-execute, discord-send, disk-usage
- docker-exec, docker-logs, docker-stats
- email-send, env-get, env-list, event-emit
- file-analyzer, file-delete, file-download, file-move, file-read, file-search, file-upload, file-write
- gateway-config, gateway-query, gateway-restart, gateway-update
- git-branch, git-commit, git-diff, git-log, git-status
- graph-query, health-check, heartbeat-send
- http-request, http-webhook
- image-analyze, image-generate
- lock-acquire, lock-release, log-write
- memory-read, memory-write, message-send, metric-track
- network-stats, nodes-list, notification-send
- performance-measure, pipeline-execute, pubsub-publish, pubsub-subscribe
- queue-consume, queue-publish
- ratelimit-check, retry-execute
- saga-execute, schedule-create, semaphore-acquire, semaphore-release
- session-history, session-list, session-spawn, session-status
- slack-send, stream-create, stream-pipe
- task-schedule, telegram-send, template-render, text-to-speech
- throttle-execute, trace-log, transform-map
- web-fetch, web-search, webhook-send, whatsapp-send
- workflow-start, workflow-status

### Native Ops (26 Rust-based)
- archive-tar, archive-zip, disk-usage, docker-exec, docker-logs, docker-ps, docker-stats
- echo, env-get
- file-delete, file-hash, file-metadata, file-move, file-search, file-watch, file-write
- git-branch, git-commit, git-diff, git-log, git-status
- network-stats, process-list
- sql-query, sql-schema, system-healthcheck

## High-Priority THUMMIM Skills to Port (Next 60 skills to reach 300/50%)

### Communication & Messaging (15 skills)
- [ ] apple-mail (14 items) - Email management
- [ ] outlook (6 items) - Microsoft email
- [ ] gmail - Google email integration
- [ ] protonmail (3 items) - Secure email
- [ ] imap-email (5 items) - Generic IMAP
- [ ] bluebubbles - iMessage bridge
- [ ] beeper - Multi-messenger
- [ ] signal - Secure messaging
- [ ] matrix - Decentralized chat
- [ ] rocketchat - Team chat
- [ ] mattermost - Team collaboration
- [ ] zulip - Threaded chat
- [ ] discord-voice (14 items) - Voice channel management
- [ ] slack-workflow - Slack automation
- [ ] teams - Microsoft Teams integration

### Calendar & Scheduling (10 skills)
- [ ] apple-calendar (8 items) - macOS calendar
- [ ] google-calendar - Google Calendar API
- [ ] outlook-calendar - Microsoft calendar
- [ ] caldav-calendar - CalDAV protocol
- [ ] calcurse - Terminal calendar
- [ ] calctl - Calendar CLI
- [ ] meeting-scheduler - Smart scheduling
- [ ] timezone-converter - Time zone handling
- [ ] availability-checker - Free/busy lookup
- [ ] calendar-sync - Multi-calendar sync

### Task & Project Management (10 skills)
- [ ] todoist - Todoist integration
- [ ] things-mac - Things 3 for Mac
- [ ] omnifocus (9 items) - OmniFocus GTD
- [ ] ticktick (13 items) - TickTick tasks
- [ ] asana (7 items) - Asana project management
- [ ] linear (2 items) - Linear issue tracking
- [ ] jira - Jira integration
- [ ] trello - Trello boards
- [ ] notion (1 items) - Notion workspace
- [ ] basecamp-cli (16 items) - Basecamp project management

### Note-Taking & Knowledge (8 skills)
- [ ] obsidian - Obsidian vault management
- [ ] apple-notes - Apple Notes integration
- [ ] bear-notes - Bear note-taking
- [ ] notion-api (2 items) - Notion API
- [ ] evernote - Evernote integration
- [ ] roam-research - Roam Research
- [ ] logseq - Logseq knowledge base
- [ ] dendron - Dendron notes

### Cloud Storage & Files (7 skills)
- [ ] dropbox (4 items) - Dropbox integration
- [ ] google-drive - Google Drive API
- [ ] onedrive - Microsoft OneDrive
- [ ] icloud-drive - iCloud Drive
- [ ] box - Box.com storage
- [ ] s3-storage - AWS S3 operations
- [ ] r2-upload (10 items) - Cloudflare R2

### Developer Tools (10 skills)
- [ ] github-pr (3 items) - GitHub pull requests
- [ ] gitlab - GitLab integration
- [ ] bitbucket - Bitbucket repos
- [ ] vercel-deploy - Vercel deployment
- [ ] netlify - Netlify deployment
- [ ] railway - Railway.app deployment
- [ ] fly.io - Fly.io deployment
- [ ] docker-compose - Docker Compose orchestration
- [ ] kubernetes (7 items) - K8s cluster management
- [ ] terraform - Infrastructure as code

## Medium-Priority Skills (Next 90 skills to reach 390/65%)

### Social Media (15 skills)
- [ ] twitter/x - Twitter/X posting
- [ ] bluesky (8 items) - Bluesky social
- [ ] mastodon - Mastodon federation
- [ ] linkedin - LinkedIn integration
- [ ] facebook - Facebook API
- [ ] instagram - Instagram posting
- [ ] tiktok - TikTok integration
- [ ] reddit (3 items) - Reddit API
- [ ] hackernews - Hacker News
- [ ] producthunt - Product Hunt
- [ ] youtube - YouTube API
- [ ] twitch - Twitch streaming
- [ ] spotify (2 items) - Spotify integration
- [ ] soundcloud - SoundCloud API
- [ ] medium - Medium publishing

### Finance & Payments (12 skills)
- [ ] stripe - Stripe payments
- [ ] paypal - PayPal integration
- [ ] plaid - Bank account linking
- [ ] mint - Mint financial data
- [ ] ynab - YNAB budgeting
- [ ] quickbooks - QuickBooks accounting
- [ ] wave - Wave accounting
- [ ] coinbase - Cryptocurrency
- [ ] kraken (4 items) - Crypto exchange
- [ ] stock-analysis (6 items) - Stock market data
- [ ] yahoo-finance - Financial data
- [ ] alpha-vantage - Market data API

### E-commerce & Shopping (10 skills)
- [ ] shopify - Shopify store management
- [ ] woocommerce - WooCommerce integration
- [ ] amazon - Amazon API
- [ ] ebay - eBay marketplace
- [ ] etsy - Etsy shop management
- [ ] square - Square POS
- [ ] stripe-checkout - Stripe checkout
- [ ] paypal-checkout - PayPal checkout
- [ ] cart-abandonment - Cart recovery
- [ ] product-recommendations - Product suggestions

### Analytics & Monitoring (13 skills)
- [ ] google-analytics - GA4 integration
- [ ] mixpanel - Mixpanel analytics
- [ ] amplitude - Amplitude product analytics
- [ ] segment - Segment CDP
- [ ] datadog - Datadog monitoring
- [ ] newrelic - New Relic APM
- [ ] sentry - Sentry error tracking
- [ ] bugsnag - Bugsnag monitoring
- [ ] rollbar - Rollbar error tracking
- [ ] loggly - Loggly log management
- [ ] splunk - Splunk analytics
- [ ] elasticsearch - Elasticsearch queries
- [ ] prometheus - Prometheus metrics

### CRM & Sales (10 skills)
- [ ] salesforce - Salesforce CRM
- [ ] hubspot - HubSpot CRM
- [ ] pipedrive - Pipedrive sales
- [ ] zoho-crm - Zoho CRM
- [ ] freshsales - Freshsales CRM
- [ ] intercom - Intercom messaging
- [ ] drift - Drift chat
- [ ] zendesk - Zendesk support
- [ ] freshdesk - Freshdesk support
- [ ] helpscout - Help Scout

### Content & Media (15 skills)
- [ ] wordpress - WordPress API
- [ ] contentful - Contentful CMS
- [ ] sanity - Sanity CMS
- [ ] strapi - Strapi headless CMS
- [ ] ghost - Ghost publishing
- [ ] substack - Substack newsletter
- [ ] mailchimp - Mailchimp email marketing
- [ ] sendgrid - SendGrid email
- [ ] twilio - Twilio SMS/voice
- [ ] vonage - Vonage communications
- [ ] cloudinary - Cloudinary media
- [ ] imgix - Imgix image processing
- [ ] vimeo - Vimeo video
- [ ] wistia - Wistia video hosting
- [ ] mux - Mux video streaming

### AI & ML Services (15 skills)
- [ ] openai-gpt4 - GPT-4 API
- [ ] anthropic-claude - Claude API
- [ ] google-palm - PaLM API
- [ ] cohere - Cohere API
- [ ] huggingface - HuggingFace models
- [ ] replicate - Replicate AI
- [ ] stability-ai - Stable Diffusion
- [ ] midjourney - Midjourney (unofficial)
- [ ] dall-e - DALL-E image generation
- [ ] whisper-api - Whisper transcription
- [ ] elevenlabs - ElevenLabs TTS
- [ ] play-ht - Play.ht voice
- [ ] resemble-ai - Resemble AI voice
- [ ] synthesia - Synthesia video
- [ ] runway - Runway ML

## Lower-Priority / Specialized Skills (Remaining ~210 to reach 600/100%)

### Smart Home & IoT (30 skills)
- [ ] homeassistant - Home Assistant
- [ ] homey (22 items) - Homey smart home
- [ ] philips-hue - Philips Hue lights
- [ ] nest-devices (3 items) - Google Nest
- [ ] ring - Ring doorbell
- [ ] ecobee - Ecobee thermostat
- [ ] smartthings - Samsung SmartThings
- [ ] ifttt - IFTTT automation
- [ ] zapier - Zapier workflows
- [ ] alexa - Amazon Alexa
- [ ] google-home - Google Home
- [ ] siri-shortcuts - Siri Shortcuts
- [ ] tesla (2 items) - Tesla vehicle control
- [ ] govee-lights (3 items) - Govee LED
- [ ] nanoleaf - Nanoleaf panels
- [ ] sonos - Sonos audio
- [ ] roku (8 items) - Roku streaming
- [ ] chromecast - Chromecast control
- [ ] apple-tv - Apple TV control
- [ ] samsung-tv - Samsung Smart TV
- [ ] lg-webos - LG webOS TV
- [ ] harmony - Logitech Harmony
- [ ] broadlink - Broadlink IR
- [ ] tuya - Tuya smart devices
- [ ] wyze - Wyze cameras
- [ ] arlo - Arlo security
- [ ] unifi (10 items) - UniFi networking
- [ ] pihole (3 items) - Pi-hole DNS
- [ ] tailscale (3 items) - Tailscale VPN
- [ ] wireguard - WireGuard VPN

### Health & Fitness (25 skills)
- [ ] apple-health - Apple Health data
- [ ] fitbit - Fitbit integration
- [ ] garmin - Garmin Connect
- [ ] strava (3 items) - Strava activities
- [ ] myfitnesspal - MyFitnessPal nutrition
- [ ] cronometer - Cronometer tracking
- [ ] whoop (2 items) - WHOOP recovery
- [ ] oura (1 items) - Oura ring data
- [ ] withings - Withings devices
- [ ] polar - Polar fitness
- [ ] suunto - Suunto watches
- [ ] peloton - Peloton workouts
- [ ] zwift - Zwift cycling
- [ ] runkeeper - RunKeeper
- [ ] nike-run - Nike Run Club
- [ ] headspace - Headspace meditation
- [ ] calm - Calm meditation
- [ ] sleep-cycle - Sleep tracking
- [ ] autosleep - AutoSleep data
- [ ] pillow - Pillow sleep tracker
- [ ] lose-it - Lose It! app
- [ ] noom - Noom coaching
- [ ] weight-watchers - WW app
- [ ] carb-manager - Carb Manager
- [ ] zero-fasting - Zero fasting app

### Entertainment & Media (20 skills)
- [ ] plex - Plex media server
- [ ] jellyfin - Jellyfin media
- [ ] emby - Emby server
- [ ] sonarr (2 items) - Sonarr TV automation
- [ ] radarr (2 items) - Radarr movie automation
- [ ] lidarr - Lidarr music automation
- [ ] readarr - Readarr book automation
- [ ] prowlarr (3 items) - Prowlarr indexer
- [ ] overseerr (7 items) - Overseerr requests
- [ ] tautulli - Tautulli Plex stats
- [ ] ombi - Ombi requests
- [ ] trakt - Trakt.tv tracking
- [ ] letterboxd - Letterboxd movies
- [ ] goodreads - Goodreads books
- [ ] audible - Audible audiobooks
- [ ] kindle - Kindle library
- [ ] pocket - Pocket read later
- [ ] instapaper (4 items) - Instapaper
- [ ] readwise (3 items) - Readwise highlights
- [ ] raindrop (2 items) - Raindrop bookmarks

### Travel & Transportation (15 skills)
- [ ] google-maps - Google Maps API
- [ ] mapbox - Mapbox mapping
- [ ] uber - Uber rides
- [ ] lyft - Lyft rides
- [ ] flight-tracker (3 items) - Flight tracking
- [ ] flightradar24 - Flight radar
- [ ] tripadvisor - TripAdvisor reviews
- [ ] booking-com - Booking.com
- [ ] airbnb - Airbnb integration
- [ ] expedia - Expedia travel
- [ ] kayak - Kayak search
- [ ] skyscanner - Skyscanner flights
- [ ] rome2rio - Rome2rio routing
- [ ] citymapper - Citymapper transit
- [ ] moovit - Moovit public transit

### Gaming & Entertainment (15 skills)
- [ ] steam - Steam gaming
- [ ] epic-games - Epic Games Store
- [ ] gog - GOG.com
- [ ] xbox - Xbox Live
- [ ] playstation - PlayStation Network
- [ ] nintendo-switch - Nintendo eShop
- [ ] twitch-api - Twitch API
- [ ] discord-bot - Discord bot framework
- [ ] minecraft-server - Minecraft server
- [ ] factorio - Factorio integration
- [ ] satisfactory - Satisfactory game
- [ ] valheim - Valheim server
- [ ] terraria - Terraria server
- [ ] starbound - Starbound server
- [ ] ark-survival - ARK server

### Education & Learning (10 skills)
- [ ] coursera - Coursera courses
- [ ] udemy - Udemy learning
- [ ] skillshare - Skillshare classes
- [ ] linkedin-learning - LinkedIn Learning
- [ ] pluralsight - Pluralsight tech
- [ ] khan-academy - Khan Academy
- [ ] duolingo - Duolingo language
- [ ] anki - Anki flashcards
- [ ] quizlet - Quizlet study
- [ ] memrise - Memrise language

### Weather & Environment (10 skills)
- [ ] openweather - OpenWeather API
- [ ] weatherapi - WeatherAPI.com
- [ ] darksky - Dark Sky (deprecated)
- [ ] accuweather - AccuWeather
- [ ] weather-underground - Weather Underground
- [ ] noaa - NOAA weather
- [ ] met-office - UK Met Office
- [ ] environment-canada - Environment Canada
- [ ] air-quality - Air quality index
- [ ] pollen-count - Pollen forecast

### Specialized/Niche (remaining ~95 skills)
- Various personal automation skills
- Regional/local service integrations
- Custom business workflows
- Experimental/prototype skills
- Legacy/deprecated skills

---

## Migration Priority Ranking

### Tier 1 (Immediate - Next 30 skills)
Focus on high-value, broadly applicable skills that enhance core functionality.

### Tier 2 (Short-term - Next 60 skills)
Popular integrations and commonly requested features.

### Tier 3 (Medium-term - Next 90 skills)
Specialized but valuable integrations for specific use cases.

### Tier 4 (Long-term - Remaining ~210 skills)
Niche, experimental, or lower-demand skills.

---

*This inventory will be updated as skills are migrated from THUMMIM to CARNELIAN.*
