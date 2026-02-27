import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface AirtableRecordParams {
  action: 'create' | 'update' | 'get' | 'list' | 'delete' | 'search';
  baseId?: string;
  tableName?: string;
  recordId?: string;
  fields?: Record<string, unknown>;
  filterByFormula?: string;
  limit?: number;
}

export async function execute(
  context: SkillContext,
  params: AirtableRecordParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return { success: false, error: 'Gateway connection not available' };
  }

  try {
    const response = await gateway.call('airtable.record', {
      action: params.action,
      baseId: params.baseId,
      tableName: params.tableName,
      recordId: params.recordId,
      fields: params.fields,
      filterByFormula: params.filterByFormula,
      limit: params.limit || 100,
    });

    return { success: true, data: response };
  } catch (error) {
    return { success: false, error: error instanceof Error ? error.message : 'Failed to execute Airtable record action' };
  }
}
