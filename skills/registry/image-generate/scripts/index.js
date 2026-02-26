/**
 * image-generate skill wrapper
 * Category: creative
 * Ported from THUMMIM: image-tool.ts
 *
 * Sandbox globals available: fetch, URL, URLSearchParams, process.env
 * Required env vars: OPENAI_API_KEY
 */

module.exports.run = async (input) => {
  // Validate input
  const prompt = input.prompt;
  
  if (!prompt) {
    throw new Error("Missing required field: prompt");
  }
  
  // Resolve API key
  const apiKey = process.env.OPENAI_API_KEY;
  if (!apiKey) {
    throw new Error("OPENAI_API_KEY environment variable is not set");
  }
  
  // Build request body
  const body = {
    prompt,
    model: input.model || "dall-e-3",
    size: input.size || "1024x1024",
    quality: input.quality || "standard",
    n: input.n || 1,
    response_format: input.response_format || "url",
  };
  
  // Call OpenAI API
  const response = await fetch("https://api.openai.com/v1/images/generations", {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      "Authorization": `Bearer ${apiKey}`,
    },
    body: JSON.stringify(body),
  });
  
  if (!response.ok) {
    const errorText = await response.text();
    throw new Error(`OpenAI API error (${response.status}): ${errorText}`);
  }
  
  const data = await response.json();
  
  // Return based on response format
  if (body.response_format === "url") {
    return {
      url: data.data[0].url,
      revised_prompt: data.data[0].revised_prompt,
    };
  } else {
    return {
      b64_json: data.data[0].b64_json,
      revised_prompt: data.data[0].revised_prompt,
    };
  }
};
