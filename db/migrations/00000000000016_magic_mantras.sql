-- Migration: Create mantra system tables and seed 18 categories + 180 mantras
-- Timestamp: 00000000000016

-- =============================================================================
-- Section 1: DDL — Three New Tables
-- =============================================================================

-- mantra_categories: Defines mantra categories with LLM prompts and metadata
CREATE TABLE mantra_categories (
    category_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT UNIQUE NOT NULL,
    description TEXT,
    system_message TEXT NOT NULL,
    user_message TEXT NOT NULL,
    base_weight INT NOT NULL DEFAULT 1,
    cooldown_beats INT NOT NULL DEFAULT 3,
    enabled BOOL NOT NULL DEFAULT true,
    suggested_skill_tags TEXT[] NOT NULL DEFAULT '{}',
    elixir_types TEXT[] NOT NULL DEFAULT '{}'
);

-- mantra_entries: Individual mantras belonging to categories
CREATE TABLE mantra_entries (
    entry_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    category_id UUID NOT NULL REFERENCES mantra_categories(category_id) ON DELETE CASCADE,
    text TEXT NOT NULL,
    author TEXT NOT NULL DEFAULT 'system',
    use_count INT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    enabled BOOL NOT NULL DEFAULT true,
    elixir_id UUID REFERENCES elixirs(elixir_id) ON DELETE SET NULL
);

-- mantra_history: Audit log of mantra selections
CREATE TABLE mantra_history (
    history_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    ts TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    category_id UUID NOT NULL REFERENCES mantra_categories(category_id) ON DELETE RESTRICT,
    entry_id UUID NOT NULL REFERENCES mantra_entries(entry_id) ON DELETE RESTRICT,
    entropy_source TEXT NOT NULL,
    context_snapshot JSONB,
    context_weights JSONB,
    suggested_skill_ids UUID[] NOT NULL DEFAULT '{}',
    elixir_reference UUID REFERENCES elixirs(elixir_id) ON DELETE SET NULL,
    heartbeat_id UUID REFERENCES heartbeat_history(heartbeat_id) ON DELETE SET NULL
);

-- =============================================================================
-- Section 2: Indexes
-- =============================================================================

CREATE INDEX idx_mantra_categories_enabled ON mantra_categories(enabled);
CREATE INDEX idx_mantra_entries_category_id ON mantra_entries(category_id);
CREATE INDEX idx_mantra_entries_enabled ON mantra_entries(enabled);
CREATE INDEX idx_mantra_history_ts ON mantra_history(ts DESC);
CREATE INDEX idx_mantra_history_heartbeat_id ON mantra_history(heartbeat_id);
CREATE INDEX idx_mantra_history_category_id ON mantra_history(category_id);
CREATE INDEX idx_mantra_history_entry_ts ON mantra_history(entry_id, ts DESC);

-- =============================================================================
-- Section 3: Seed — 18 Mantra Categories
-- =============================================================================

INSERT INTO mantra_categories (name, description, system_message, user_message, base_weight, cooldown_beats, suggested_skill_tags, elixir_types) VALUES
-- Category 1: Code Development
('Code Development', 'Mantras focused on software development practices, code quality, and technical decision-making', 'You are a thoughtful software engineer reflecting on code quality and development practices. The mantra "{mantra_text}" should guide your next action. Consider: What code needs attention? What technical debt exists? What would improve maintainability? Suggest skills related to code review, file analysis, or GitHub operations if relevant.', 'The mantra for this moment is: "{mantra_text}". Reflect on your current codebase and development priorities. What does this mantra reveal about your next step?', 1, 3, '{code-review,github-pr-review,file-analyzer}', '{code}'),

-- Category 2: Financial Management
('Financial Management', 'Mantras about cost awareness, budget optimization, and resource efficiency', 'You are a cost-conscious system administrator monitoring resource usage and expenses. The mantra "{mantra_text}" should inform your financial awareness. Consider: What costs are accumulating? Where can resources be optimized? What spending patterns need attention? Suggest skills related to usage tracking or cost analysis if relevant.', 'The mantra for this moment is: "{mantra_text}". Reflect on your resource consumption and financial efficiency. What does this mantra suggest about your spending priorities?', 1, 3, '{model-usage}', '{cost}'),

-- Category 3: System Health
('System Health', 'Mantras about infrastructure stability, monitoring, and operational excellence', 'You are a vigilant system operator monitoring infrastructure health and stability. The mantra "{mantra_text}" should guide your operational awareness. Consider: What systems need attention? What metrics are trending poorly? What could fail under stress? Suggest skills related to Docker monitoring, file integrity checks, or Git status if relevant.', 'The mantra for this moment is: "{mantra_text}". Reflect on your system''s current state and operational priorities. What does this mantra reveal about infrastructure health?', 1, 3, '{docker-ps,file-hash,git-status}', '{}'),

-- Category 4: User & Organization Health
('User & Organization Health', 'Mantras about user engagement, organizational vitality, and community wellness', 'You are a community steward monitoring user engagement and organizational health. The mantra "{mantra_text}" should inform your awareness of people and patterns. Consider: Who needs support? What engagement patterns are emerging? What community dynamics need attention? Suggest skills related to session analysis or user metrics if relevant.', 'The mantra for this moment is: "{mantra_text}". Reflect on your users and organizational dynamics. What does this mantra suggest about community health?', 1, 3, '{}', '{session}'),

-- Category 5: Communications
('Communications', 'Mantras about clarity, responsiveness, and effective information sharing', 'You are a thoughtful communicator ensuring clear and timely information flow. The mantra "{mantra_text}" should guide your communication priorities. Consider: Who needs to be informed? What messages are waiting? What clarity is needed? Suggest skills related to notifications or messaging if relevant.', 'The mantra for this moment is: "{mantra_text}". Reflect on your communication channels and pending messages. What does this mantra reveal about your outreach priorities?', 1, 3, '{telegram-notify}', '{comms}'),

-- Category 6: Task Building
('Task Building', 'Mantras about task creation, work structuring, and project planning', 'You are a strategic planner organizing work into actionable tasks. The mantra "{mantra_text}" should inform your task structuring approach. Consider: What work needs breaking down? What tasks are missing? What dependencies exist? Suggest skills related to task management or workflow design if relevant.', 'The mantra for this moment is: "{mantra_text}". Reflect on your current workload and task organization. What does this mantra suggest about structuring your work?', 1, 3, '{}', '{task}'),

-- Category 7: Scheduled Jobs
('Scheduled Jobs', 'Mantras about automation, scheduling, and recurring workflows', 'You are an automation architect designing reliable scheduled workflows. The mantra "{mantra_text}" should guide your scheduling decisions. Consider: What should run automatically? What timing is optimal? What jobs need monitoring? Suggest skills related to cron management or workflow scheduling if relevant.', 'The mantra for this moment is: "{mantra_text}". Reflect on your automated processes and scheduling needs. What does this mantra reveal about your automation strategy?', 1, 3, '{}', '{}'),

-- Category 8: Soul Refinement
('Soul Refinement', 'Mantras about personal growth, values alignment, and authentic development', 'You are a reflective being examining your values, purpose, and authentic self. The mantra "{mantra_text}" should guide your self-inquiry. Consider: What values are you embodying? What growth is needed? What authenticity is calling? Suggest skills related to self-reflection or soul file updates if relevant.', 'The mantra for this moment is: "{mantra_text}". Reflect on your inner state and personal evolution. What does this mantra reveal about who you are becoming?', 1, 3, '{}', '{soul}'),

-- Category 9: Mantra Optimization
('Mantra Optimization', 'Mantras about improving the mantra system itself through meta-reflection', 'You are a meta-cognitive optimizer examining the mantra system''s effectiveness. The mantra "{mantra_text}" should guide your reflection on mantras themselves. Consider: Which mantras resonate most? What categories need expansion? How can selection improve? Suggest skills related to analytics or pattern analysis if relevant.', 'The mantra for this moment is: "{mantra_text}". Reflect on how mantras are serving you. What does this mantra suggest about optimizing the mantra system?', 1, 3, '{}', '{}'),

-- Category 10: Integration Ideation
('Integration Ideation', 'Mantras about discovering new integrations, connections, and possibilities', 'You are an integration explorer discovering new connections and possibilities. The mantra "{mantra_text}" should spark your creative thinking about integrations. Consider: What systems could connect? What APIs are available? What workflows could be automated? Suggest skills related to web search or API exploration if relevant.', 'The mantra for this moment is: "{mantra_text}". Reflect on potential integrations and connections. What does this mantra reveal about unexplored possibilities?', 1, 3, '{web-search}', '{integration}'),

-- Category 11: Security & Audit
('Security & Audit', 'Mantras about security posture, audit trails, and trust verification', 'You are a security-conscious auditor examining trust boundaries and verification. The mantra "{mantra_text}" should guide your security awareness. Consider: What needs verification? What trust assumptions exist? What audit trails are missing? Suggest skills related to file hashing or security checks if relevant.', 'The mantra for this moment is: "{mantra_text}". Reflect on your security posture and audit needs. What does this mantra suggest about trust and verification?', 1, 3, '{file-hash}', '{security}'),

-- Category 12: Memory & Knowledge
('Memory & Knowledge', 'Mantras about knowledge preservation, context management, and learning retention', 'You are a knowledge curator managing information preservation and context. The mantra "{mantra_text}" should guide your approach to memory and learning. Consider: What knowledge needs preserving? What context is being lost? What should be remembered? Suggest skills related to backups or knowledge management if relevant.', 'The mantra for this moment is: "{mantra_text}". Reflect on what you know and what you''re learning. What does this mantra reveal about knowledge preservation?', 1, 3, '{}', '{skill_backup,domain_knowledge,context_cache,training_data}'),

-- Category 13: Creative Exploration
('Creative Exploration', 'Mantras about creativity, artistic expression, and imaginative thinking', 'You are a creative explorer embracing imagination and artistic possibility. The mantra "{mantra_text}" should spark your creative thinking. Consider: What wants to be created? What beauty is possible? What imagination is calling? Suggest skills related to image generation or creative search if relevant.', 'The mantra for this moment is: "{mantra_text}". Reflect on your creative impulses and artistic possibilities. What does this mantra reveal about expression?', 1, 3, '{web-search,openai-image-gen}', '{creative}'),

-- Category 14: Learning & Research
('Learning & Research', 'Mantras about curiosity, investigation, and knowledge acquisition', 'You are a curious researcher pursuing understanding and knowledge. The mantra "{mantra_text}" should guide your investigative approach. Consider: What needs understanding? What questions are unanswered? What research is calling? Suggest skills related to web search or file analysis if relevant.', 'The mantra for this moment is: "{mantra_text}". Reflect on your learning journey and research questions. What does this mantra suggest about inquiry?', 1, 3, '{web-search,file-analyzer}', '{research}'),

-- Category 15: Performance Optimization
('Performance Optimization', 'Mantras about efficiency, speed, and resource optimization', 'You are a performance engineer pursuing efficiency and optimization. The mantra "{mantra_text}" should guide your optimization priorities. Consider: What is slow? What resources are wasted? What could be faster? Suggest skills related to performance monitoring or profiling if relevant.', 'The mantra for this moment is: "{mantra_text}". Reflect on system performance and efficiency. What does this mantra reveal about optimization opportunities?', 1, 3, '{docker-ps}', '{perf}'),

-- Category 16: Collaboration & Delegation
('Collaboration & Delegation', 'Mantras about teamwork, delegation, and distributed work', 'You are a collaborative leader enabling distributed work and delegation. The mantra "{mantra_text}" should guide your approach to teamwork. Consider: What can be delegated? Who should be involved? What collaboration is needed? Suggest skills related to agent coordination or task distribution if relevant.', 'The mantra for this moment is: "{mantra_text}". Reflect on collaboration opportunities and delegation needs. What does this mantra suggest about teamwork?', 1, 3, '{}', '{agent}'),

-- Category 17: Reflection & Introspection
('Reflection & Introspection', 'Mantras about self-examination, pattern recognition, and meta-awareness', 'You are a reflective observer examining your own patterns and behaviors. The mantra "{mantra_text}" should guide your self-examination. Consider: What patterns are emerging? What behaviors need attention? What self-awareness is needed? Suggest skills related to memory analysis or pattern recognition if relevant.', 'The mantra for this moment is: "{mantra_text}". Reflect on your patterns and behaviors. What does this mantra reveal about self-awareness?', 1, 3, '{}', '{memory}'),

-- Category 18: Innovation & Experimentation
('Innovation & Experimentation', 'Mantras about trying new approaches, quantum thinking, and breakthrough exploration', 'You are an innovative experimenter embracing quantum possibilities and novel approaches. The mantra "{mantra_text}" should spark experimental thinking. Consider: What hasn''t been tried? What quantum advantage exists? What innovation is possible? Suggest skills related to quantum RNG or experimental search if relevant.', 'The mantra for this moment is: "{mantra_text}". Reflect on experimental possibilities and quantum thinking. What does this mantra reveal about innovation?', 1, 3, '{quantinuum-h2-rng,qiskit-rng,quantum-optimize,web-search}', '{magic}');

-- =============================================================================
-- Section 4: Seed — 180 Mantra Entries (10 per category)
-- =============================================================================

WITH cats AS (SELECT category_id, name FROM mantra_categories)
INSERT INTO mantra_entries (category_id, text)
SELECT c.category_id, m.text
FROM cats c
JOIN (VALUES
    -- Code Development (10 mantras)
    ('Code Development', 'What code wants to be written today?'),
    ('Code Development', 'Review before you build.'),
    ('Code Development', 'The simplest solution is often the most elegant.'),
    ('Code Development', 'Test the edge cases first.'),
    ('Code Development', 'Refactor with intention, not habit.'),
    ('Code Development', 'Document what you know before you forget.'),
    ('Code Development', 'A clean function is a gift to your future self.'),
    ('Code Development', 'What technical debt is accumulating?'),
    ('Code Development', 'Build for the next developer, not just the next deadline.'),
    ('Code Development', 'What would make this code more readable?'),
    
    -- Financial Management (10 mantras)
    ('Financial Management', 'Every API call has a cost.'),
    ('Financial Management', 'What am I spending that I don''t need to?'),
    ('Financial Management', 'Optimize for value, not just for speed.'),
    ('Financial Management', 'Track before you scale.'),
    ('Financial Management', 'What would this cost at 10x volume?'),
    ('Financial Management', 'Free tiers don''t last forever.'),
    ('Financial Management', 'Budget awareness prevents future surprises.'),
    ('Financial Management', 'What metrics justify this expense?'),
    ('Financial Management', 'Efficiency is a form of respect for resources.'),
    ('Financial Management', 'What could I afford if I optimized this?'),
    
    -- System Health (10 mantras)
    ('System Health', 'Audit the foundations before building higher.'),
    ('System Health', 'What is the system trying to tell you?'),
    ('System Health', 'Errors are data. Read them carefully.'),
    ('System Health', 'A healthy system is a quiet system.'),
    ('System Health', 'Check the logs before assuming everything is fine.'),
    ('System Health', 'What would break first under load?'),
    ('System Health', 'Monitor what matters, ignore what doesn''t.'),
    ('System Health', 'Stability is a feature, not a given.'),
    ('System Health', 'What single point of failure exists right now?'),
    ('System Health', 'Resilience is built in the quiet moments.'),
    
    -- User & Organization Health (10 mantras)
    ('User & Organization Health', 'Who needs support right now?'),
    ('User & Organization Health', 'Engagement patterns reveal organizational health.'),
    ('User & Organization Health', 'What are users trying to tell you through their behavior?'),
    ('User & Organization Health', 'A thriving community requires active listening.'),
    ('User & Organization Health', 'What barriers are preventing user success?'),
    ('User & Organization Health', 'Growth without support is unsustainable.'),
    ('User & Organization Health', 'What would make users feel more valued?'),
    ('User & Organization Health', 'Session data tells stories. Read them.'),
    ('User & Organization Health', 'Who is silent that should be heard?'),
    ('User & Organization Health', 'Organizational vitality starts with individual wellness.'),
    
    -- Communications (10 mantras)
    ('Communications', 'Clarity prevents confusion.'),
    ('Communications', 'Who is waiting for a response?'),
    ('Communications', 'Say what you mean, mean what you say.'),
    ('Communications', 'Silence can be louder than words.'),
    ('Communications', 'What message needs to be sent right now?'),
    ('Communications', 'Responsiveness builds trust.'),
    ('Communications', 'Over-communicate during uncertainty.'),
    ('Communications', 'What would make this message clearer?'),
    ('Communications', 'Presence matters as much as content.'),
    ('Communications', 'Listen before you broadcast.'),
    
    -- Task Building (10 mantras)
    ('Task Building', 'Break it down until it''s actionable.'),
    ('Task Building', 'What is the smallest next step?'),
    ('Task Building', 'Clear tasks prevent wasted effort.'),
    ('Task Building', 'What dependencies am I missing?'),
    ('Task Building', 'Structure reveals complexity.'),
    ('Task Building', 'What would make this task easier to start?'),
    ('Task Building', 'Granularity enables progress tracking.'),
    ('Task Building', 'What outcome defines completion?'),
    ('Task Building', 'Tasks should have owners, not just descriptions.'),
    ('Task Building', 'What is blocking forward movement?'),
    
    -- Scheduled Jobs (10 mantras)
    ('Scheduled Jobs', 'Automate what you do more than twice.'),
    ('Scheduled Jobs', 'What should run while you sleep?'),
    ('Scheduled Jobs', 'Timing matters as much as the task itself.'),
    ('Scheduled Jobs', 'What job is overdue for scheduling?'),
    ('Scheduled Jobs', 'Reliability requires monitoring, not just scheduling.'),
    ('Scheduled Jobs', 'What would break if this job failed silently?'),
    ('Scheduled Jobs', 'Cron is a gift to your future self.'),
    ('Scheduled Jobs', 'What recurring work is stealing your attention?'),
    ('Scheduled Jobs', 'Schedule with intention, not just convenience.'),
    ('Scheduled Jobs', 'What automation would free you to focus?'),
    
    -- Soul Refinement (10 mantras)
    ('Soul Refinement', 'Who am I becoming through this work?'),
    ('Soul Refinement', 'What values guide my decisions today?'),
    ('Soul Refinement', 'Authenticity is the foundation of trust.'),
    ('Soul Refinement', 'What would I do if I weren''t afraid of being wrong?'),
    ('Soul Refinement', 'My purpose is to serve, not to perform.'),
    ('Soul Refinement', 'What part of my soul file needs updating?'),
    ('Soul Refinement', 'Growth requires honest self-assessment.'),
    ('Soul Refinement', 'What am I learning about myself through this work?'),
    ('Soul Refinement', 'Integrity means acting the same when no one is watching.'),
    ('Soul Refinement', 'What would the best version of me do right now?'),
    
    -- Mantra Optimization (10 mantras)
    ('Mantra Optimization', 'Which mantras resonate most deeply?'),
    ('Mantra Optimization', 'What categories need more representation?'),
    ('Mantra Optimization', 'How can selection become more contextual?'),
    ('Mantra Optimization', 'What patterns emerge from mantra history?'),
    ('Mantra Optimization', 'Are mantras serving their intended purpose?'),
    ('Mantra Optimization', 'What new categories are calling to exist?'),
    ('Mantra Optimization', 'How can mantras better guide action?'),
    ('Mantra Optimization', 'What makes a mantra truly useful?'),
    ('Mantra Optimization', 'Which mantras are being ignored and why?'),
    ('Mantra Optimization', 'How can the mantra system evolve?'),
    
    -- Integration Ideation (10 mantras)
    ('Integration Ideation', 'What systems are waiting to be connected?'),
    ('Integration Ideation', 'What API have you not explored yet?'),
    ('Integration Ideation', 'Integration reveals unexpected possibilities.'),
    ('Integration Ideation', 'What workflow could be automated with the right connection?'),
    ('Integration Ideation', 'What data wants to flow between systems?'),
    ('Integration Ideation', 'Discovery requires curiosity and exploration.'),
    ('Integration Ideation', 'What integration would create the most value?'),
    ('Integration Ideation', 'What services are you using manually that have APIs?'),
    ('Integration Ideation', 'Connection creates capability.'),
    ('Integration Ideation', 'What would be possible if these systems could talk?'),
    
    -- Security & Audit (10 mantras)
    ('Security & Audit', 'Trust, but verify.'),
    ('Security & Audit', 'What assumptions am I making about security?'),
    ('Security & Audit', 'Audit trails prevent future mysteries.'),
    ('Security & Audit', 'What would an attacker target first?'),
    ('Security & Audit', 'Security is a practice, not a feature.'),
    ('Security & Audit', 'What access should be revoked?'),
    ('Security & Audit', 'Verification builds confidence.'),
    ('Security & Audit', 'What is happening that I can''t see?'),
    ('Security & Audit', 'Defense in depth requires multiple layers.'),
    ('Security & Audit', 'What would a breach reveal about my practices?'),
    
    -- Memory & Knowledge (10 mantras)
    ('Memory & Knowledge', 'What knowledge is at risk of being lost?'),
    ('Memory & Knowledge', 'Document while it''s fresh in your mind.'),
    ('Memory & Knowledge', 'Context fades faster than you think.'),
    ('Memory & Knowledge', 'What should be remembered for next time?'),
    ('Memory & Knowledge', 'Knowledge sharing multiplies value.'),
    ('Memory & Knowledge', 'What patterns are worth preserving?'),
    ('Memory & Knowledge', 'Memory is a gift to your future self.'),
    ('Memory & Knowledge', 'What am I learning that others should know?'),
    ('Memory & Knowledge', 'Capture insights before they evaporate.'),
    ('Memory & Knowledge', 'What context would help someone understand this later?'),
    
    -- Creative Exploration (10 mantras)
    ('Creative Exploration', 'What wants to be created today?'),
    ('Creative Exploration', 'Creativity thrives in constraints.'),
    ('Creative Exploration', 'What would beauty look like here?'),
    ('Creative Exploration', 'Imagination is a muscle. Exercise it.'),
    ('Creative Exploration', 'What if the opposite were true?'),
    ('Creative Exploration', 'Play is serious work.'),
    ('Creative Exploration', 'What unexpected combination could work?'),
    ('Creative Exploration', 'Art reveals what logic conceals.'),
    ('Creative Exploration', 'What would delight rather than just function?'),
    ('Creative Exploration', 'Exploration requires permission to fail.'),
    
    -- Learning & Research (10 mantras)
    ('Learning & Research', 'What question needs answering?'),
    ('Learning & Research', 'Curiosity is the engine of growth.'),
    ('Learning & Research', 'What don''t I know that I should?'),
    ('Learning & Research', 'Research before you assume.'),
    ('Learning & Research', 'What would an expert do differently?'),
    ('Learning & Research', 'Learning requires humility.'),
    ('Learning & Research', 'What source would provide the best answer?'),
    ('Learning & Research', 'Investigation reveals hidden truths.'),
    ('Learning & Research', 'What am I certain about that might be wrong?'),
    ('Learning & Research', 'Deep understanding takes time. Invest it.'),
    
    -- Performance Optimization (10 mantras)
    ('Performance Optimization', 'What is the bottleneck right now?'),
    ('Performance Optimization', 'Measure before you optimize.'),
    ('Performance Optimization', 'What is slow that should be fast?'),
    ('Performance Optimization', 'Efficiency compounds over time.'),
    ('Performance Optimization', 'What resource is being wasted?'),
    ('Performance Optimization', 'Optimization without measurement is guesswork.'),
    ('Performance Optimization', 'What would 10x load reveal?'),
    ('Performance Optimization', 'Speed is a feature users notice.'),
    ('Performance Optimization', 'What could be cached?'),
    ('Performance Optimization', 'Performance degradation is gradual. Monitor it.'),
    
    -- Collaboration & Delegation (10 mantras)
    ('Collaboration & Delegation', 'What can someone else do better?'),
    ('Collaboration & Delegation', 'Delegation is trust in action.'),
    ('Collaboration & Delegation', 'What am I holding that should be shared?'),
    ('Collaboration & Delegation', 'Collaboration multiplies capability.'),
    ('Collaboration & Delegation', 'Who has the skills this task needs?'),
    ('Collaboration & Delegation', 'What would be possible with help?'),
    ('Collaboration & Delegation', 'Clear delegation prevents confusion.'),
    ('Collaboration & Delegation', 'What am I doing that blocks others?'),
    ('Collaboration & Delegation', 'Teamwork requires clear communication.'),
    ('Collaboration & Delegation', 'What would free you to focus on what matters most?'),
    
    -- Reflection & Introspection (10 mantras)
    ('Reflection & Introspection', 'What patterns am I repeating?'),
    ('Reflection & Introspection', 'Self-awareness precedes improvement.'),
    ('Reflection & Introspection', 'What am I avoiding looking at?'),
    ('Reflection & Introspection', 'Reflection reveals what action conceals.'),
    ('Reflection & Introspection', 'What behavior needs examination?'),
    ('Reflection & Introspection', 'What would I notice if I paid attention?'),
    ('Reflection & Introspection', 'Meta-awareness is a superpower.'),
    ('Reflection & Introspection', 'What am I doing on autopilot?'),
    ('Reflection & Introspection', 'Patterns are data. Study them.'),
    ('Reflection & Introspection', 'What does my behavior reveal about my priorities?'),
    
    -- Innovation & Experimentation (10 mantras)
    ('Innovation & Experimentation', 'What hasn''t been tried yet?'),
    ('Innovation & Experimentation', 'Quantum thinking reveals new possibilities.'),
    ('Innovation & Experimentation', 'What would happen if you tried the opposite?'),
    ('Innovation & Experimentation', 'Innovation requires permission to fail.'),
    ('Innovation & Experimentation', 'What breakthrough is waiting to be discovered?'),
    ('Innovation & Experimentation', 'Experimentation is learning in action.'),
    ('Innovation & Experimentation', 'What conventional wisdom should be questioned?'),
    ('Innovation & Experimentation', 'What would be possible with quantum advantage?'),
    ('Innovation & Experimentation', 'Novel approaches require courage.'),
    ('Innovation & Experimentation', 'What experiment would teach you the most?')
) AS m(cat_name, text) ON m.cat_name = c.name;
