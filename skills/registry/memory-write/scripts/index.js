/**
 * memory-write skill wrapper
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
  const content = input.content;
  
  if (!inputPath) {
    throw new Error("Missing required field: path");
  }
  
  if (content === undefined || content === null) {
    throw new Error("Missing required field: content");
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
  
  // Ensure parent directory exists
  const dir = path.dirname(resolvedPath);
  await fs.mkdir(dir, { recursive: true });
  
  // Write or append to file
  if (input.append) {
    await fs.appendFile(resolvedPath, content, "utf-8");
  } else {
    await fs.writeFile(resolvedPath, content, "utf-8");
  }
  
  // Calculate bytes written
  const bytesWritten = Buffer.byteLength(content, "utf-8");
  
  return {
    ok: true,
    path: resolvedPath,
    bytesWritten,
  };
};
