#!/usr/bin/env node
// Mock worker for testing ProcessJsonlTransport
//
// Reads JSON Lines from stdin, processes requests, writes JSON Lines to stdout.
//
// Environment variables:
//   MOCK_WORKER_SLEEP_MS     - Delay before responding (default: 0)
//   MOCK_WORKER_OUTPUT_SIZE  - Size of output payload in bytes (default: 0, uses echo)
//   MOCK_WORKER_EMIT_EVENTS  - Number of StreamEvent messages to emit before response (default: 0)

const readline = require('readline');
const crypto = require('crypto');

const SLEEP_MS = parseInt(process.env.MOCK_WORKER_SLEEP_MS || '0', 10);
const OUTPUT_SIZE = parseInt(process.env.MOCK_WORKER_OUTPUT_SIZE || '0', 10);
const EMIT_EVENTS = parseInt(process.env.MOCK_WORKER_EMIT_EVENTS || '0', 10);

const rl = readline.createInterface({ input: process.stdin, terminal: false });

function sleep(ms) {
  return new Promise(resolve => setTimeout(resolve, ms));
}

function writeJsonLine(obj) {
  process.stdout.write(JSON.stringify(obj) + '\n');
}

// Queue-based serial processing to prevent async readline race conditions.
// Without this, if SLEEP_MS > 0, multiple lines could be processed concurrently
// because readline doesn't await the async callback.
const lineQueue = [];
let processing = false;

async function processQueue() {
  if (processing) return;
  processing = true;
  while (lineQueue.length > 0) {
    const line = lineQueue.shift();
    await handleLine(line);
  }
  processing = false;
}

async function handleLine(line) {
  let msg;
  try {
    msg = JSON.parse(line);
  } catch (e) {
    process.stderr.write(`Failed to parse: ${line}\n`);
    return;
  }

  if (msg.type === 'Invoke') {
    const { message_id, payload } = msg;
    const { run_id, skill_name, input } = payload;
    const startTime = Date.now();

    // Emit stream events if configured
    for (let i = 0; i < EMIT_EVENTS; i++) {
      writeJsonLine({
        type: 'Stream',
        message_id: crypto.randomUUID(),
        payload: {
          run_id,
          event_type: i === 0 ? 'Log' : 'Progress',
          timestamp: new Date().toISOString(),
          level: 'Info',
          message: `Event ${i + 1}/${EMIT_EVENTS}`,
          fields: { index: i },
        },
      });
    }

    // Sleep if configured
    if (SLEEP_MS > 0) {
      await sleep(SLEEP_MS);
    }

    // Build result payload
    let result;
    let truncated = false;
    if (OUTPUT_SIZE > 0) {
      // Generate large output
      result = { data: 'x'.repeat(OUTPUT_SIZE) };
    } else {
      // Echo input as result
      result = { echo: input, skill_name };
    }

    const durationMs = Date.now() - startTime;

    writeJsonLine({
      type: 'InvokeResult',
      message_id: crypto.randomUUID(),
      payload: {
        run_id,
        status: 'Success',
        result,
        error: null,
        exit_code: 0,
        duration_ms: durationMs,
        truncated,
      },
    });
  } else if (msg.type === 'Cancel') {
    const { payload } = msg;
    process.stderr.write(`Cancellation received for ${JSON.stringify(payload.run_id)}: ${payload.reason}\n`);
    process.exit(0);
  } else if (msg.type === 'Health') {
    writeJsonLine({
      type: 'HealthResult',
      message_id: msg.message_id,
      payload: {
        healthy: true,
        worker_id: process.env.WORKER_ID || 'mock-worker',
        uptime_secs: Math.floor(process.uptime()),
      },
    });
  }
}

rl.on('line', (line) => {
  lineQueue.push(line);
  processQueue();
});

rl.on('close', () => {
  process.exit(0);
});
