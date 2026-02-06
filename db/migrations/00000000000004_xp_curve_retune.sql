-- =============================================================================
-- Migration 0004: XP Curve Retune
-- =============================================================================
-- Retunes the level_progression table from exponent 1.15 to 1.172.
--
-- Old curve (1.15): ~680M total XP at level 99
-- New curve (1.172): ~3.88B total XP at level 99
--
-- This migration:
-- 1. Truncates and reseeds level_progression with the new exponent
-- 2. Recalculates level and xp_to_next_level for existing agent_xp rows
-- 3. Updates the default value for agent_xp.xp_to_next_level
-- =============================================================================

-- Step 1: Truncate existing level_progression data
TRUNCATE level_progression;

-- Step 2: Reseed level_progression with exponent 1.172
WITH levels AS (
    SELECT
        level,
        CASE
            WHEN level = 1 THEN 0::bigint
            ELSE floor(100 * power(1.172::numeric, level - 1))::bigint
        END AS xp_required
    FROM generate_series(1, 99) AS level
),
cumulative AS (
    SELECT
        level,
        xp_required,
        sum(xp_required) OVER (ORDER BY level) AS total_xp_required
    FROM levels
)
INSERT INTO level_progression (level, xp_required, total_xp_required, milestone_feature)
SELECT
    level,
    xp_required,
    total_xp_required,
    CASE level
        WHEN 5  THEN 'unlock_sub_agents'
        WHEN 10 THEN 'unlock_workflows'
        WHEN 15 THEN 'unlock_external_channels'
        WHEN 20 THEN 'unlock_voice'
        WHEN 25 THEN 'master_tier_badge'
        WHEN 50 THEN 'grandmaster_tier'
        WHEN 75 THEN 'legend_tier'
        WHEN 99 THEN 'max_level_achieved'
        ELSE NULL
    END
FROM cumulative;

-- Step 3: Recalculate level and xp_to_next_level for existing agent_xp rows
UPDATE agent_xp
SET
    level = (
        SELECT COALESCE(MAX(lp.level), 1)
        FROM level_progression lp
        WHERE lp.total_xp_required <= agent_xp.total_xp
    ),
    xp_to_next_level = (
        SELECT COALESCE(
            (SELECT lp2.total_xp_required - agent_xp.total_xp
             FROM level_progression lp2
             WHERE lp2.level = (
                 SELECT COALESCE(MAX(lp.level), 1) + 1
                 FROM level_progression lp
                 WHERE lp.total_xp_required <= agent_xp.total_xp
             )),
            0
        )
    );

-- Step 4: Update default value for xp_to_next_level to match new level 2 threshold
ALTER TABLE agent_xp
ALTER COLUMN xp_to_next_level
SET DEFAULT (SELECT total_xp_required FROM level_progression WHERE level = 2);
