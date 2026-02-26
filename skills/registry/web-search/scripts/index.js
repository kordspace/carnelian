/**
 * web-search skill wrapper
 * Category: research
 * Ported from THUMMIM: web-search.ts
 *
 * Sandbox globals available: fetch, URL, URLSearchParams, process.env
 * Required env vars: BRAVE_API_KEY (Brave) or PERPLEXITY_API_KEY/OPENROUTER_API_KEY (Perplexity)
 */

module.exports.run = async (input) => {
  const startTime = Date.now();
  
  // Parse input
  const provider = input.provider || "brave";
  const query = input.query;
  const count = Math.min(input.count || 5, 10);
  const country = input.country;
  const searchLang = input.search_lang;
  const uiLang = input.ui_lang;
  const freshness = input.freshness;
  
  if (!query) {
    throw new Error("Missing required field: query");
  }
  
  if (provider === "brave") {
    return await searchBrave({
      query,
      count,
      country,
      searchLang,
      uiLang,
      freshness,
      startTime,
    });
  } else if (provider === "perplexity") {
    return await searchPerplexity({
      query,
      startTime,
    });
  } else {
    throw new Error(`Unknown provider: ${provider}`);
  }
};

async function searchBrave({ query, count, country, searchLang, uiLang, freshness, startTime }) {
  const apiKey = process.env.BRAVE_API_KEY;
  if (!apiKey) {
    throw new Error("BRAVE_API_KEY environment variable is not set");
  }
  
  // Build query parameters
  const params = new URLSearchParams({
    q: query,
    count: String(count),
  });
  
  if (country) params.set("country", country);
  if (searchLang) params.set("search_lang", searchLang);
  if (uiLang) params.set("ui_lang", uiLang);
  if (freshness) params.set("freshness", freshness);
  
  const url = `https://api.search.brave.com/res/v1/web/search?${params.toString()}`;
  
  const response = await fetch(url, {
    method: "GET",
    headers: {
      "Accept": "application/json",
      "Accept-Encoding": "gzip",
      "X-Subscription-Token": apiKey,
    },
  });
  
  if (!response.ok) {
    const errorText = await response.text();
    throw new Error(`Brave Search API error (${response.status}): ${errorText}`);
  }
  
  const data = await response.json();
  const results = (data.web?.results || []).map((r) => ({
    title: r.title || "",
    url: r.url || "",
    description: r.description || "",
    published: r.page_age || r.published,
    siteName: r.profile?.name,
  }));
  
  return {
    query,
    provider: "brave",
    count: results.length,
    tookMs: Date.now() - startTime,
    results,
  };
}

async function searchPerplexity({ query, startTime }) {
  // Try PERPLEXITY_API_KEY first, then OPENROUTER_API_KEY
  let apiKey = process.env.PERPLEXITY_API_KEY || process.env.OPENROUTER_API_KEY;
  if (!apiKey) {
    throw new Error("PERPLEXITY_API_KEY or OPENROUTER_API_KEY environment variable is not set");
  }
  
  // Infer base URL from key prefix
  let baseUrl;
  if (apiKey.startsWith("pplx-")) {
    baseUrl = "https://api.perplexity.ai";
  } else if (apiKey.startsWith("sk-or-")) {
    baseUrl = "https://openrouter.ai/api/v1";
  } else {
    // Default to Perplexity
    baseUrl = "https://api.perplexity.ai";
  }
  
  const model = "perplexity/sonar-pro";
  
  const response = await fetch(`${baseUrl}/chat/completions`, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      "Authorization": `Bearer ${apiKey}`,
    },
    body: JSON.stringify({
      model,
      messages: [
        {
          role: "user",
          content: query,
        },
      ],
    }),
  });
  
  if (!response.ok) {
    const errorText = await response.text();
    throw new Error(`Perplexity API error (${response.status}): ${errorText}`);
  }
  
  const data = await response.json();
  const content = data.choices?.[0]?.message?.content || "";
  const citations = data.citations || [];
  
  return {
    query,
    provider: "perplexity",
    model,
    tookMs: Date.now() - startTime,
    content,
    citations,
  };
}
