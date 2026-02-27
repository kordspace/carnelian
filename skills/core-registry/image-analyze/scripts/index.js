/**
 * image-analyze skill wrapper
 * Category: creative
 * Ported from THUMMIM: image-tool.ts
 *
 * Sandbox globals available: fetch, URL, URLSearchParams, process.env
 * Required env vars: OPENAI_API_KEY or ANTHROPIC_API_KEY
 */

module.exports.run = async (input) => {
  // Validate input
  const image = input.image;
  
  if (!image) {
    throw new Error("Missing required field: image");
  }
  
  // Validate image is a URL (not a file path)
  if (!image.startsWith("http://") && !image.startsWith("https://") && !image.startsWith("data:")) {
    throw new Error("Image must be an HTTP(S) URL or data: URL. File paths are not supported in the sandbox.");
  }
  
  const prompt = input.prompt || "Describe the image.";
  
  // Determine provider
  const openaiKey = process.env.OPENAI_API_KEY;
  const anthropicKey = process.env.ANTHROPIC_API_KEY;
  
  let provider = input.provider;
  if (!provider) {
    provider = openaiKey ? "openai" : anthropicKey ? "anthropic" : null;
  }
  
  if (!provider) {
    throw new Error("No API key available. Set OPENAI_API_KEY or ANTHROPIC_API_KEY environment variable.");
  }
  
  if (provider === "openai") {
    return await analyzeWithOpenAI(image, prompt, input.model, openaiKey);
  } else if (provider === "anthropic") {
    return await analyzeWithAnthropic(image, prompt, input.model, anthropicKey);
  } else {
    throw new Error(`Unknown provider: ${provider}`);
  }
};

async function analyzeWithOpenAI(image, prompt, model, apiKey) {
  if (!apiKey) {
    throw new Error("OPENAI_API_KEY environment variable is not set");
  }
  
  const modelName = model || "gpt-4o";
  
  const response = await fetch("https://api.openai.com/v1/chat/completions", {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      "Authorization": `Bearer ${apiKey}`,
    },
    body: JSON.stringify({
      model: modelName,
      messages: [
        {
          role: "user",
          content: [
            {
              type: "text",
              text: prompt,
            },
            {
              type: "image_url",
              image_url: {
                url: image,
              },
            },
          ],
        },
      ],
      max_tokens: 1000,
    }),
  });
  
  if (!response.ok) {
    const errorText = await response.text();
    throw new Error(`OpenAI API error (${response.status}): ${errorText}`);
  }
  
  const data = await response.json();
  
  // Normalize content to string
  let content = data.choices[0].message.content;
  let text;
  
  if (Array.isArray(content)) {
    // If content is an array of parts, extract text fields
    text = content.map(part => {
      if (typeof part === "string") {
        return part;
      } else if (part.text) {
        return part.text;
      }
      return "";
    }).join("");
  } else {
    // If content is already a string, use it as-is
    text = content;
  }
  
  return {
    text,
    model: modelName,
    provider: "openai",
  };
}

async function analyzeWithAnthropic(image, prompt, model, apiKey) {
  if (!apiKey) {
    throw new Error("ANTHROPIC_API_KEY environment variable is not set");
  }
  
  const modelName = model || "claude-opus-4-5";
  
  // Convert data URLs to base64 format for Anthropic
  let imageSource;
  if (image.startsWith("data:")) {
    const match = image.match(/^data:image\/(\w+);base64,(.+)$/);
    if (!match) {
      throw new Error("Invalid data URL format");
    }
    imageSource = {
      type: "base64",
      media_type: `image/${match[1]}`,
      data: match[2],
    };
  } else {
    // For HTTP URLs, fetch and convert to base64
    const imageResponse = await fetch(image);
    if (!imageResponse.ok) {
      throw new Error(`Failed to fetch image: ${imageResponse.status}`);
    }
    const arrayBuffer = await imageResponse.arrayBuffer();
    const base64 = Buffer.from(arrayBuffer).toString("base64");
    const contentType = imageResponse.headers.get("content-type") || "image/jpeg";
    imageSource = {
      type: "base64",
      media_type: contentType,
      data: base64,
    };
  }
  
  const response = await fetch("https://api.anthropic.com/v1/messages", {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      "x-api-key": apiKey,
      "anthropic-version": "2023-06-01",
    },
    body: JSON.stringify({
      model: modelName,
      max_tokens: 1000,
      messages: [
        {
          role: "user",
          content: [
            {
              type: "image",
              source: imageSource,
            },
            {
              type: "text",
              text: prompt,
            },
          ],
        },
      ],
    }),
  });
  
  if (!response.ok) {
    const errorText = await response.text();
    throw new Error(`Anthropic API error (${response.status}): ${errorText}`);
  }
  
  const data = await response.json();
  
  return {
    text: data.content[0].text,
    model: modelName,
    provider: "anthropic",
  };
}
