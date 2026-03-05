import type { SkillContext, SkillResult } from '../../types';

interface AirtableCreateRecordParams {
  baseId: string;
  tableName: string;
  fields: Record<string, any>;
}

export async function execute(
  context: SkillContext,
  params: AirtableCreateRecordParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.baseId || !params.tableName || !params.fields) {
    return {
      success: false,
      error: 'baseId, tableName, and fields are required',
    };
  }

  try {
    const response = await gateway.call('airtable.createRecord', {
      baseId: params.baseId,
      tableName: params.tableName,
      fields: params.fields,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to create Airtable record',
    };
  }
}
