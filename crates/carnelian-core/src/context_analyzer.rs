//! Context analysis and autonomous task creation
//!
//! Analyzes session context to automatically create follow-up tasks based on
//! conversation patterns, action items, and user intent.

use carnelian_common::{Error, Result};
use carnelian_magic::MantraTree;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

/// Action item extracted from conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionItem {
    /// Title of the action
    pub title: String,
    /// Detailed description
    pub description: String,
    /// Priority level (1-5, 5 being highest)
    pub priority: i32,
    /// Estimated complexity (1-5)
    pub complexity: i32,
    /// Suggested skills to use
    pub suggested_skills: Vec<String>,
    /// Context from conversation
    pub context: JsonValue,
}

/// Context analyzer for autonomous task creation
pub struct ContextAnalyzer {
    pool: Arc<PgPool>,
    mantra_tree: Arc<MantraTree>,
}

impl ContextAnalyzer {
    /// Create a new context analyzer
    pub fn new(pool: Arc<PgPool>, mantra_tree: Arc<MantraTree>) -> Self {
        Self { pool, mantra_tree }
    }

    /// Analyze a session and extract action items
    ///
    /// # Arguments
    /// * `session_id` - The session to analyze
    /// * `message_limit` - Number of recent messages to analyze (default: 10)
    ///
    /// # Returns
    /// A list of extracted action items
    pub async fn analyze_session(
        &self,
        session_id: Uuid,
        message_limit: i64,
    ) -> Result<Vec<ActionItem>> {
        // Fetch recent messages from session
        let messages = self
            .fetch_recent_messages(session_id, message_limit)
            .await?;

        if messages.is_empty() {
            return Ok(Vec::new());
        }

        // Extract action items using pattern matching
        let action_items = self.extract_action_items(&messages).await?;

        Ok(action_items)
    }

    /// Fetch recent messages from a session
    async fn fetch_recent_messages(&self, session_id: Uuid, limit: i64) -> Result<Vec<String>> {
        let rows: Vec<(String,)> = sqlx::query_as(
            r#"
            SELECT content
            FROM session_messages
            WHERE session_id = $1
            ORDER BY ts DESC
            LIMIT $2
            "#,
        )
        .bind(session_id)
        .bind(limit)
        .fetch_all(self.pool.as_ref())
        .await
        .map_err(Error::Database)?;

        Ok(rows.into_iter().map(|r| r.0).collect())
    }

    /// Extract action items from messages using pattern matching
    async fn extract_action_items(&self, messages: &[String]) -> Result<Vec<ActionItem>> {
        let mut action_items = Vec::new();

        // Combine messages into context
        let context = messages.join("\n\n");

        // Pattern matching for common action item indicators
        let patterns = [
            ("TODO:", 3),
            ("FIXME:", 4),
            ("need to", 2),
            ("should", 2),
            ("must", 4),
            ("implement", 3),
            ("create", 2),
            ("add", 2),
            ("update", 2),
            ("fix", 3),
            ("deploy", 4),
            ("test", 3),
        ];

        for (pattern, priority) in patterns {
            if context.to_lowercase().contains(&pattern.to_lowercase()) {
                // Extract context around the pattern
                if let Some(item) = self.extract_item_from_pattern(&context, pattern, priority) {
                    action_items.push(item);
                }
            }
        }

        // Deduplicate similar items
        self.deduplicate_items(&mut action_items);

        Ok(action_items)
    }

    /// Extract an action item from a pattern match
    fn extract_item_from_pattern(
        &self,
        context: &str,
        pattern: &str,
        priority: i32,
    ) -> Option<ActionItem> {
        // Find the pattern in context
        let lower_context = context.to_lowercase();
        let pattern_lower = pattern.to_lowercase();

        if let Some(pos) = lower_context.find(&pattern_lower) {
            // Extract surrounding context (up to 200 chars after pattern)
            let start = pos;
            let end = (pos + 200).min(context.len());
            let excerpt = &context[start..end];

            // Extract first sentence as title
            let title = excerpt
                .lines()
                .next()
                .unwrap_or(excerpt)
                .trim()
                .chars()
                .take(100)
                .collect::<String>();

            if title.len() < 10 {
                return None; // Too short to be meaningful
            }

            Some(ActionItem {
                title: title.clone(),
                description: excerpt.to_string(),
                priority,
                complexity: 3, // Default medium complexity
                suggested_skills: Vec::new(),
                context: serde_json::json!({
                    "pattern": pattern,
                    "excerpt": excerpt,
                }),
            })
        } else {
            None
        }
    }

    /// Deduplicate similar action items
    fn deduplicate_items(&self, items: &mut Vec<ActionItem>) {
        // Simple deduplication based on title similarity
        let mut seen_titles = std::collections::HashSet::new();
        items.retain(|item| {
            let normalized = item.title.to_lowercase().trim().to_string();
            seen_titles.insert(normalized)
        });
    }

    /// Create tasks from action items
    ///
    /// # Arguments
    /// * `session_id` - The session these tasks belong to
    /// * `action_items` - The action items to convert to tasks
    ///
    /// # Returns
    /// Number of tasks created
    pub async fn create_tasks_from_items(
        &self,
        session_id: Uuid,
        action_items: &[ActionItem],
    ) -> Result<usize> {
        let mut created = 0;

        for item in action_items {
            // Create task in database using existing schema columns
            let result: (Uuid,) = sqlx::query_as(
                r#"
                INSERT INTO tasks (
                    title,
                    description,
                    state,
                    priority,
                    correlation_id,
                    created_at
                ) VALUES ($1, $2, $3, $4, $5, NOW())
                RETURNING task_id
                "#,
            )
            .bind(&item.title)
            .bind(&item.description)
            .bind("pending")
            .bind(item.priority)
            .bind(session_id)
            .fetch_one(self.pool.as_ref())
            .await
            .map_err(Error::Database)?;

            created += 1;
            tracing::info!(task_id = %result.0, title = %item.title, "Created task from action item");
        }

        Ok(created)
    }

    /// Analyze session and auto-create tasks
    ///
    /// Convenience method that combines analysis and task creation.
    ///
    /// # Arguments
    /// * `session_id` - The session to analyze
    ///
    /// # Returns
    /// Number of tasks created
    pub async fn analyze_and_create_tasks(&self, session_id: Uuid) -> Result<usize> {
        let action_items = self.analyze_session(session_id, 10).await?;

        if action_items.is_empty() {
            return Ok(0);
        }

        self.create_tasks_from_items(session_id, &action_items)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper function to create a test analyzer without requiring a real PgPool
    fn create_test_context_analyzer() -> (Arc<PgPool>, ContextAnalyzer) {
        // Use connect_lazy to create a non-functional but safe pool for pure logic tests
        let pool = Arc::new(PgPool::connect_lazy("postgres://localhost/test").unwrap());
        let mantra_tree = Arc::new(MantraTree::new(None));
        let analyzer = ContextAnalyzer::new(pool.clone(), mantra_tree);
        (pool, analyzer)
    }

    #[tokio::test]
    async fn test_extract_item_from_pattern() {
        let (_pool, analyzer) = create_test_context_analyzer();

        let context = "We need to implement the new authentication system with OAuth2 support";
        let item = analyzer.extract_item_from_pattern(context, "implement", 3);

        assert!(item.is_some());
        let item = item.unwrap();
        assert!(item.title.contains("implement"));
        assert_eq!(item.priority, 3);
    }

    #[tokio::test]
    async fn test_deduplicate_items() {
        let (_pool, analyzer) = create_test_context_analyzer();

        let mut items = vec![
            ActionItem {
                title: "Fix the bug".to_string(),
                description: "desc1".to_string(),
                priority: 3,
                complexity: 2,
                suggested_skills: vec![],
                context: serde_json::json!({}),
            },
            ActionItem {
                title: "Fix the bug".to_string(),
                description: "desc2".to_string(),
                priority: 3,
                complexity: 2,
                suggested_skills: vec![],
                context: serde_json::json!({}),
            },
        ];

        analyzer.deduplicate_items(&mut items);
        assert_eq!(items.len(), 1); // One duplicate removed
    }
}
