import { loadConfig } from "./config.js";
import { GatewayServer } from "./server.js";
import { log } from "./utils.js";

// =============================================================================
// ENTRY POINT
// =============================================================================

async function main(): Promise<void> {
  log("info", "Carnelian Gateway starting...");

  // Load configuration (env → file → defaults)
  const config = loadConfig();

  log("info", "Configuration loaded", {
    port: config.port,
    coreApiUrl: config.coreApiUrl,
    localFirst: config.routing.localFirst,
    fallbackEnabled: config.routing.fallbackEnabled,
    providers: {
      ollama: config.providers.ollama.enabled,
      openai: config.providers.openai.enabled,
      anthropic: config.providers.anthropic.enabled,
      fireworks: config.providers.fireworks.enabled,
    },
  });

  const server = new GatewayServer(config);

  // Graceful shutdown
  const shutdown = async (signal: string) => {
    log("info", `Received ${signal}, shutting down...`);
    try {
      await server.stop();
      process.exit(0);
    } catch (err) {
      log("error", "Error during shutdown", { error: String(err) });
      process.exit(1);
    }
  };

  process.on("SIGTERM", () => void shutdown("SIGTERM"));
  process.on("SIGINT", () => void shutdown("SIGINT"));

  // Start server
  try {
    await server.start();
  } catch (err) {
    log("error", "Failed to start gateway", { error: String(err) });
    process.exit(1);
  }
}

main().catch((err) => {
  log("error", "Fatal error", { error: String(err) });
  process.exit(1);
});
