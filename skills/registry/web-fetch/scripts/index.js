/**
 * web-fetch skill wrapper
 * Category: research
 * Ported from THUMMIM: web-fetch.ts
 *
 * Sandbox globals available: fetch, URL, URLSearchParams, process.env
 * Required env vars: FIRECRAWL_API_KEY (optional)
 */

module.exports.run = async (input) => {
  const startTime = Date.now();
  
  // Parse input
  const url = input.url;
  const extractMode = input.extractMode || "markdown";
  const maxChars = input.maxChars || 50000;
  
  if (!url) {
    throw new Error("Missing required field: url");
  }
  
  // Try Firecrawl if API key is available
  const firecrawlKey = process.env.FIRECRAWL_API_KEY;
  if (firecrawlKey) {
    return await fetchWithFirecrawl({ url, extractMode, maxChars, startTime, firecrawlKey });
  }
  
  // Fall back to direct fetch
  return await fetchDirect({ url, extractMode, maxChars, startTime });
};

async function fetchWithFirecrawl({ url, extractMode, maxChars, startTime, firecrawlKey }) {
  const response = await fetch("https://api.firecrawl.dev/v2/scrape", {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      "Authorization": `Bearer ${firecrawlKey}`,
    },
    body: JSON.stringify({
      url,
      formats: ["markdown"],
      onlyMainContent: true,
    }),
  });
  
  if (!response.ok) {
    const errorText = await response.text();
    throw new Error(`Firecrawl API error (${response.status}): ${errorText}`);
  }
  
  const data = await response.json();
  let text = data.data?.markdown || "";
  const title = data.data?.metadata?.title;
  
  // Convert markdown to plain text if extractMode is "text"
  if (extractMode === "text") {
    text = stripMarkdown(text);
  }
  
  const truncated = text.length > maxChars;
  if (truncated) {
    text = text.substring(0, maxChars);
  }
  
  return {
    url,
    finalUrl: url,
    status: 200,
    contentType: extractMode === "markdown" ? "text/markdown" : "text/plain",
    title,
    extractMode,
    extractor: "firecrawl",
    truncated,
    length: text.length,
    fetchedAt: new Date().toISOString(),
    tookMs: Date.now() - startTime,
    text,
  };
}

async function fetchDirect({ url, extractMode, maxChars, startTime }) {
  const response = await fetch(url, {
    method: "GET",
    headers: {
      "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
      "Accept": "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
    },
  });
  
  if (!response.ok) {
    throw new Error(`HTTP error (${response.status}): ${response.statusText}`);
  }
  
  const contentType = response.headers.get("content-type") || "";
  let text = await response.text();
  let title;
  let processedContentType = contentType;
  
  // Extract title from HTML
  if (contentType.includes("text/html")) {
    const titleMatch = text.match(/<title[^>]*>([^<]+)<\/title>/i);
    if (titleMatch) {
      title = titleMatch[1].trim();
    }
    
    if (extractMode === "markdown") {
      // Basic HTML to markdown conversion
      text = htmlToMarkdown(text);
      processedContentType = "text/markdown";
    } else {
      // Strip HTML tags for plain text
      text = text.replace(/<script[^>]*>[\s\S]*?<\/script>/gi, "");
      text = text.replace(/<style[^>]*>[\s\S]*?<\/style>/gi, "");
      text = text.replace(/<[^>]+>/g, " ");
      text = text.replace(/\s+/g, " ").trim();
      processedContentType = "text/plain";
    }
  } else if (contentType.includes("application/json")) {
    // Pretty-print JSON
    try {
      const parsed = JSON.parse(text);
      text = JSON.stringify(parsed, null, 2);
    } catch (e) {
      // Keep as-is if not valid JSON
    }
  }
  
  const truncated = text.length > maxChars;
  if (truncated) {
    text = text.substring(0, maxChars);
  }
  
  return {
    url,
    finalUrl: response.url,
    status: response.status,
    contentType: processedContentType,
    title,
    extractMode,
    extractor: "direct",
    truncated,
    length: text.length,
    fetchedAt: new Date().toISOString(),
    tookMs: Date.now() - startTime,
    text,
  };
}

// Helper: Strip markdown formatting to plain text
function stripMarkdown(text) {
  return text
    .replace(/^#{1,6}\s+/gm, "") // Headers
    .replace(/\*\*(.+?)\*\*/g, "$1") // Bold
    .replace(/\*(.+?)\*/g, "$1") // Italic
    .replace(/__(.+?)__/g, "$1") // Bold
    .replace(/_(.+?)_/g, "$1") // Italic
    .replace(/~~(.+?)~~/g, "$1") // Strikethrough
    .replace(/`(.+?)`/g, "$1") // Inline code
    .replace(/```[\s\S]*?```/g, "") // Code blocks
    .replace(/\[(.+?)\]\(.+?\)/g, "$1") // Links
    .replace(/!\[.*?\]\(.+?\)/g, "") // Images
    .replace(/^\s*[-*+]\s+/gm, "") // Unordered lists
    .replace(/^\s*\d+\.\s+/gm, "") // Ordered lists
    .replace(/^\s*>\s+/gm, "") // Blockquotes
    .replace(/\n{3,}/g, "\n\n") // Multiple newlines
    .trim();
}

// Helper: Basic HTML to markdown conversion
function htmlToMarkdown(html) {
  // Remove scripts and styles
  let text = html.replace(/<script[^>]*>[\s\S]*?<\/script>/gi, "");
  text = text.replace(/<style[^>]*>[\s\S]*?<\/style>/gi, "");
  
  // Convert common HTML elements to markdown
  text = text.replace(/<h1[^>]*>(.*?)<\/h1>/gi, "# $1\n\n");
  text = text.replace(/<h2[^>]*>(.*?)<\/h2>/gi, "## $1\n\n");
  text = text.replace(/<h3[^>]*>(.*?)<\/h3>/gi, "### $1\n\n");
  text = text.replace(/<h4[^>]*>(.*?)<\/h4>/gi, "#### $1\n\n");
  text = text.replace(/<h5[^>]*>(.*?)<\/h5>/gi, "##### $1\n\n");
  text = text.replace(/<h6[^>]*>(.*?)<\/h6>/gi, "###### $1\n\n");
  text = text.replace(/<strong[^>]*>(.*?)<\/strong>/gi, "**$1**");
  text = text.replace(/<b[^>]*>(.*?)<\/b>/gi, "**$1**");
  text = text.replace(/<em[^>]*>(.*?)<\/em>/gi, "*$1*");
  text = text.replace(/<i[^>]*>(.*?)<\/i>/gi, "*$1*");
  text = text.replace(/<code[^>]*>(.*?)<\/code>/gi, "`$1`");
  text = text.replace(/<a[^>]*href=["']([^"']+)["'][^>]*>(.*?)<\/a>/gi, "[$2]($1)");
  text = text.replace(/<img[^>]*src=["']([^"']+)["'][^>]*alt=["']([^"']+)["'][^>]*>/gi, "![$2]($1)");
  text = text.replace(/<img[^>]*src=["']([^"']+)["'][^>]*>/gi, "![]($1)");
  text = text.replace(/<br\s*\/?>/gi, "\n");
  text = text.replace(/<p[^>]*>(.*?)<\/p>/gi, "$1\n\n");
  text = text.replace(/<li[^>]*>(.*?)<\/li>/gi, "- $1\n");
  text = text.replace(/<ul[^>]*>(.*?)<\/ul>/gis, "$1\n");
  text = text.replace(/<ol[^>]*>(.*?)<\/ol>/gis, "$1\n");
  
  // Remove remaining HTML tags
  text = text.replace(/<[^>]+>/g, "");
  
  // Clean up whitespace
  text = text.replace(/\n{3,}/g, "\n\n").trim();
  
  return text;
}
