/**
 * memory-read skill wrapper
 * Category: data
 * Ported from THUMMIM: memory-tool.ts
 *
 * Sandbox globals available: fs, process.env
 * Required env vars: none
 */

import path from "node:path";

module.exports.run = async (input) => {
  // Validate input
  const inputPath = input.path;
  
  if (!inputPath) {
    throw new Error("Missing required field: path");
  }
  
  // Resolve path relative to memory directory
  const memoryDir = process.env.CARNELIAN_MEMORY_DIR || process.cwd();
  let resolvedPath;
  
  // If path is not absolute, resolve it relative to memory directory
  if (!path.isAbsolute(inputPath)) {
    resolvedPath = path.resolve(memoryDir, inputPath);
  } else {
    resolvedPath = inputPath;
  }
  
  // Read file content
  let text = await fs.readFile(resolvedPath, "utf-8");
  
  // If from/lines are specified, extract the range
  if (input.from !== undefined || input.lines !== undefined) {
    const allLines = text.split("\n");
    const from = input.from || 1;
    const lines = input.lines || allLines.length;
    
    // from is 1-indexed, so subtract 1 for array indexing
    const startIndex = from - 1;
    const endIndex = startIndex + lines;
    
    const selectedLines = allLines.slice(startIndex, endIndex);
    text = selectedLines.join("\n");
  }
  
  return {
    path: resolvedPath,
    text,
  };
};
