import { SkillContext, SkillResult } from '../../types';

export async function execute(context: SkillContext): Promise<SkillResult> {
  try {
    const { projectId, instanceName, database, query } = context.parameters;

    if (!projectId || !instanceName || !database || !query) {
      return { success: false, error: 'projectId, instanceName, database, and query are required' };
    }

    const response = await fetch(`${context.gateway_url}/gcp/cloudsql/query`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ projectId, instanceName, database, query }),
    });

    if (!response.ok) {
      return { success: false, error: `Cloud SQL API error: ${response.statusText}` };
    }

    const data = await response.json();
    return { success: true, data };
  } catch (error) {
    return { success: false, error: error instanceof Error ? error.message : 'Failed to query Cloud SQL' };
  }
}
