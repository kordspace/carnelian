import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { parseFrontmatter, validateManifest, computeChecksum } from "../manifest.js";
import type { SkillManifest } from "../manifest.js";

describe("parseFrontmatter", () => {
  it("parses valid YAML frontmatter", () => {
    const content = `---
name: test-skill
description: A test skill
---

# Body content`;

    const result = parseFrontmatter(content);
    assert.equal(result.name, "test-skill");
    assert.equal(result.description, "A test skill");
  });

  it("returns empty object when no frontmatter delimiters", () => {
    const content = "# Just markdown\nNo frontmatter here.";
    const result = parseFrontmatter(content);
    assert.deepEqual(result, {});
  });

  it("returns empty object when missing closing delimiter", () => {
    const content = `---
name: broken
description: no closing`;

    const result = parseFrontmatter(content);
    assert.deepEqual(result, {});
  });

  it("handles nested metadata objects", () => {
    const content = `---
name: nested-skill
description: Has nested metadata
metadata:
  carnelian:
    runtime: node
    version: "0.1.0"
---

# Body`;

    const result = parseFrontmatter(content);
    assert.equal(result.name, "nested-skill");
    assert.ok(result.metadata);
  });
});

describe("validateManifest", () => {
  it("validates a minimal valid manifest", () => {
    const raw = { name: "my-skill", description: "Does things" };
    const result = validateManifest(raw);
    assert.equal(result.valid, true);
    assert.equal(result.errors.length, 0);
    assert.ok(result.manifest);
    assert.equal(result.manifest!.name, "my-skill");
    assert.equal(result.manifest!.description, "Does things");
    assert.ok(result.checksum);
  });

  it("rejects manifest missing name", () => {
    const raw = { description: "No name" };
    const result = validateManifest(raw);
    assert.equal(result.valid, false);
    assert.ok(result.errors.some((e) => e.field === "name"));
  });

  it("rejects manifest missing description", () => {
    const raw = { name: "no-desc" };
    const result = validateManifest(raw);
    assert.equal(result.valid, false);
    assert.ok(result.errors.some((e) => e.field === "description"));
  });

  it("rejects manifest with empty name", () => {
    const raw = { name: "", description: "Has desc" };
    const result = validateManifest(raw);
    assert.equal(result.valid, false);
  });

  it("parses Carnelian metadata extensions", () => {
    const raw = {
      name: "extended",
      description: "Has Carnelian metadata",
      metadata: {
        carnelian: {
          runtime: "python",
          version: "1.0.0",
          sandbox: {
            network: "full",
            resourceLimits: {
              maxMemoryMB: 1024,
              maxCpuPercent: 80,
              timeoutSecs: 600,
            },
          },
          capabilities: ["net.http", "fs.read"],
        },
      },
    };

    const result = validateManifest(raw);
    assert.equal(result.valid, true);
    assert.ok(result.manifest);
    const cn = result.manifest!.metadata.carnelian;
    assert.ok(cn);
    assert.equal(cn!.runtime, "python");
    assert.equal(cn!.version, "1.0.0");
    assert.equal(cn!.sandbox.network, "full");
    assert.equal(cn!.sandbox.resourceLimits.maxMemoryMB, 1024);
    assert.deepEqual(cn!.capabilities, ["net.http", "fs.read"]);
  });

  it("parses OpenClaw metadata", () => {
    const raw = {
      name: "oc-skill",
      description: "Has OpenClaw metadata",
      metadata: {
        openclaw: {
          emoji: "🔥",
          requires: { bins: ["python3"], env: ["API_KEY"] },
          primaryEnv: "API_KEY",
          os: ["darwin"],
        },
      },
    };

    const result = validateManifest(raw);
    assert.equal(result.valid, true);
    assert.ok(result.manifest);
    const oc = result.manifest!.metadata.openclaw;
    assert.ok(oc);
    assert.equal(oc!.emoji, "🔥");
    assert.deepEqual(oc!.requires?.bins, ["python3"]);
    assert.deepEqual(oc!.requires?.env, ["API_KEY"]);
    assert.equal(oc!.primaryEnv, "API_KEY");
  });

  it("defaults Carnelian runtime to node", () => {
    const raw = {
      name: "default-runtime",
      description: "No runtime specified",
      metadata: {
        carnelian: {
          sandbox: { network: "none", resourceLimits: {} },
          capabilities: [],
        },
      },
    };

    const result = validateManifest(raw);
    assert.equal(result.valid, true);
    assert.equal(result.manifest!.metadata.carnelian!.runtime, "node");
  });
});

describe("parseFrontmatter — object arrays", () => {
  it("parses OpenClaw install array of objects", () => {
    const content = `---
name: openai-image-gen
description: Generate images via OpenAI
metadata:
  openclaw:
    emoji: "\U0001F5BC"
    requires:
      bins:
        - python3
      env:
        - OPENAI_API_KEY
    primaryEnv: OPENAI_API_KEY
    install:
      - id: python-brew
        kind: brew
        formula: python
        bins:
          - python3
        label: Install Python (brew)
---

# Body`;

    const result = parseFrontmatter(content);
    assert.equal(result.name, "openai-image-gen");
    const metadata = result.metadata as Record<string, unknown>;
    assert.ok(metadata);
    const oc = metadata.openclaw as Record<string, unknown>;
    assert.ok(oc);
    assert.equal(oc.primaryEnv, "OPENAI_API_KEY");

    // Verify install is an array of objects
    const install = oc.install as Array<Record<string, unknown>>;
    assert.ok(Array.isArray(install), "install should be an array");
    assert.equal(install.length, 1);
    assert.equal(install[0]!.id, "python-brew");
    assert.equal(install[0]!.kind, "brew");
    assert.equal(install[0]!.formula, "python");
    assert.deepEqual(install[0]!.bins, ["python3"]);
    assert.equal(install[0]!.label, "Install Python (brew)");

    // Verify requires is an object with arrays
    const requires = oc.requires as Record<string, unknown>;
    assert.deepEqual(requires.bins, ["python3"]);
    assert.deepEqual(requires.env, ["OPENAI_API_KEY"]);
  });

  it("parses model-usage manifest with shell exec capability", () => {
    const content = `---
name: model-usage
description: Summarize model usage costs
metadata:
  openclaw:
    emoji: "\U0001F4CA"
    requires:
      bins:
        - python3
        - codexbar
  carnelian:
    runtime: python
    version: "0.1.0"
    sandbox:
      network: none
      resourceLimits:
        maxMemoryMB: 256
        maxCpuPercent: 25
        timeoutSecs: 120
    capabilities:
      - shell.exec
---

# Body`;

    const result = parseFrontmatter(content);
    assert.equal(result.name, "model-usage");
    const metadata = result.metadata as Record<string, unknown>;
    const oc = metadata.openclaw as Record<string, unknown>;
    const requires = oc.requires as Record<string, unknown>;
    assert.deepEqual(requires.bins, ["python3", "codexbar"]);

    const cn = metadata.carnelian as Record<string, unknown>;
    assert.equal(cn.runtime, "python");
    assert.deepEqual(cn.capabilities, ["shell.exec"]);
  });

  it("parses multiple install instructions", () => {
    const content = `---
name: multi-install
description: Multiple install steps
metadata:
  openclaw:
    install:
      - id: python-brew
        kind: brew
        formula: python
        bins:
          - python3
        label: Install Python
      - id: ffmpeg-brew
        kind: brew
        formula: ffmpeg
        bins:
          - ffmpeg
        label: Install FFmpeg
---

# Body`;

    const result = parseFrontmatter(content);
    const metadata = result.metadata as Record<string, unknown>;
    const oc = metadata.openclaw as Record<string, unknown>;
    const install = oc.install as Array<Record<string, unknown>>;
    assert.ok(Array.isArray(install));
    assert.equal(install.length, 2);
    assert.equal(install[0]!.id, "python-brew");
    assert.equal(install[1]!.id, "ffmpeg-brew");
    assert.deepEqual(install[1]!.bins, ["ffmpeg"]);
  });
});

describe("computeChecksum", () => {
  it("produces consistent SHA-256 checksums", () => {
    const manifest: SkillManifest = {
      name: "checksum-test",
      description: "For checksum testing",
      metadata: {},
    };

    const hash1 = computeChecksum(manifest);
    const hash2 = computeChecksum(manifest);
    assert.equal(hash1, hash2);
    assert.equal(hash1.length, 64); // SHA-256 hex = 64 chars
  });

  it("produces different checksums for different manifests", () => {
    const m1: SkillManifest = { name: "a", description: "a", metadata: {} };
    const m2: SkillManifest = { name: "b", description: "b", metadata: {} };
    assert.notEqual(computeChecksum(m1), computeChecksum(m2));
  });
});
