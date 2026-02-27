/**
 * text-to-speech skill wrapper
 * Category: creative
 * Ported from THUMMIM: tts-tool.ts
 *
 * Sandbox globals available: fetch, Buffer, process.env
 * Required env vars: OPENAI_API_KEY
 */

module.exports.run = async (input) => {
  // Validate input
  const text = input.text;
  
  if (!text) {
    throw new Error("Missing required field: text");
  }
  
  // Resolve API key
  const apiKey = process.env.OPENAI_API_KEY;
  if (!apiKey) {
    throw new Error("OPENAI_API_KEY environment variable is not set");
  }
  
  // Build request body
  const model = input.model || "tts-1";
  const voice = input.voice || "alloy";
  const format = input.format || "mp3";
  
  // Call OpenAI TTS API
  const response = await fetch("https://api.openai.com/v1/audio/speech", {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      "Authorization": `Bearer ${apiKey}`,
    },
    body: JSON.stringify({
      model,
      input: text,
      voice,
      response_format: format,
    }),
  });
  
  if (!response.ok) {
    const errorText = await response.text();
    throw new Error(`OpenAI API error (${response.status}): ${errorText}`);
  }
  
  // Read response as ArrayBuffer and convert to base64
  const arrayBuffer = await response.arrayBuffer();
  const buffer = Buffer.from(arrayBuffer);
  const audio_base64 = buffer.toString("base64");
  
  return {
    audio_base64,
    format,
    provider: "openai",
  };
};
