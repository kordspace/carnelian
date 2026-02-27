import { SkillContext, SkillResult } from '../../types';

export async function execute(context: SkillContext): Promise<SkillResult> {
  try {
    const { projectId, datasetId, tableId, sourceUri, sourceFormat = 'CSV', schema } = context.parameters;

    if (!projectId || !datasetId || !tableId || !sourceUri) {
      return { success: false, error: 'projectId, datasetId, tableId, and sourceUri are required' };
    }

    const response = await fetch(`${context.gateway_url}/gcp/bigquery/load`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ projectId, datasetId, tableId, sourceUri, sourceFormat, schema }),
    });

    if (!response.ok) {
      return { success: false, error: `BigQuery API error: ${response.statusText}` };
    }

    const data = await response.json();
    return { success: true, data };
  } catch (error) {
    return { success: false, error: error instanceof Error ? error.message : 'Failed to load data into BigQuery' };
  }
}
