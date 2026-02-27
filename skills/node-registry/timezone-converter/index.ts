import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface TimezoneConverterParams {
  time: string;
  fromTimezone: string;
  toTimezone: string;
  format?: string;
}

export async function execute(
  context: SkillContext,
  params: TimezoneConverterParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.time || !params.fromTimezone || !params.toTimezone) {
    return {
      success: false,
      error: 'time, fromTimezone, and toTimezone are required',
    };
  }

  try {
    const response = await gateway.call('timezone.convert', {
      time: params.time,
      fromTimezone: params.fromTimezone,
      toTimezone: params.toTimezone,
      format: params.format || 'YYYY-MM-DD HH:mm:ss',
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to convert timezone',
    };
  }
}
