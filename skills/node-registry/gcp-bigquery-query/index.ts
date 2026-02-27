import { SkillContext, SkillResult } from '../../types';

export async function execute(context: SkillContext): Promise<SkillResult> {
  try {
    const { projectId, datasetId, query, location = 'US', maxResults = 1000 } = context.parameters;

    if (!projectId || !query) {
      return { success: false, error: 'projectId and query are required' };
    }

    const response = await fetch(`${context.gateway_url}/gcp/bigquery/query`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ projectId, datasetId, query, location, maxResults }),
    });

    if (!response.ok) {
      return { success: false, error: `BigQuery API error: ${response.statusText}` };
    }

    const data = await response.json();
    return { success: true, data };
  } catch (error) {
    return { success: false, error: error instanceof Error ? error.message : 'Failed to execute BigQuery query' };
  }
}
