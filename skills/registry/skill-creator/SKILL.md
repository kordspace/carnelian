---
name: skill-creator
description: Create or update AgentSkills. Use when designing, structuring, or packaging skills with scripts, references, and assets.
metadata:
  carnelian:
    runtime: node
    version: "0.1.0"
    sandbox:
      network: none
      resourceLimits:
        maxMemoryMB: 256
        maxCpuPercent: 25
        timeoutSecs: 120
    capabilities:
      - fs.read
      - fs.write
---

# Skill Creator

This skill provides guidance for creating effective skills.

## About Skills

Skills are modular, self-contained packages that extend capabilities by providing
specialized knowledge, workflows, and tools. They transform a general-purpose agent
into a specialized agent equipped with procedural knowledge.

### What Skills Provide

1. Specialized workflows - Multi-step procedures for specific domains
2. Tool integrations - Instructions for working with specific file formats or APIs
3. Domain expertise - Company-specific knowledge, schemas, business logic
4. Bundled resources - Scripts, references, and assets for complex and repetitive tasks

## Core Principles

### Concise is Key

The context window is a public good. Only add context the agent doesn't already have.
Challenge each piece of information: "Does the agent really need this explanation?"

### Set Appropriate Degrees of Freedom

- **High freedom**: Use when multiple approaches are valid
- **Medium freedom**: Use when a preferred pattern exists
- **Low freedom**: Use when operations are fragile and error-prone

### Anatomy of a Skill

```
skill-name/
├── SKILL.md (required)
│   ├── YAML frontmatter metadata (required)
│   │   ├── name: (required)
│   │   └── description: (required)
│   └── Markdown instructions (required)
└── Bundled Resources (optional)
    ├── scripts/          - Executable code
    ├── references/       - Documentation for context
    └── assets/           - Files used in output
```

## Skill Creation Process

1. Understand the skill with concrete examples
2. Plan reusable skill contents (scripts, references, assets)
3. Initialize the skill directory structure
4. Edit the skill (implement resources and write SKILL.md)
5. Package the skill
6. Iterate based on real usage

### Skill Naming

- Use lowercase letters, digits, and hyphens only
- Prefer short, verb-led phrases that describe the action
- Name the skill folder exactly after the skill name

### Progressive Disclosure

Skills use a three-level loading system:

1. **Metadata (name + description)** - Always in context (~100 words)
2. **SKILL.md body** - When skill triggers (<5k words)
3. **Bundled resources** - As needed (unlimited)

Keep SKILL.md body under 500 lines. Split content into separate reference files when approaching this limit.
