import type { SkillContext, SkillResult } from '../../types';

interface CalendarSyncParams {
  sourceCalendar: string;
  targetCalendar: string;
  startDate?: string;
  endDate?: string;
  syncDirection?: 'one-way' | 'two-way';
}

export async function execute(
  context: SkillContext,
  params: CalendarSyncParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.sourceCalendar || !params.targetCalendar) {
    return {
      success: false,
      error: 'sourceCalendar and targetCalendar are required',
    };
  }

  try {
    const response = await gateway.call('calendar.sync', {
      sourceCalendar: params.sourceCalendar,
      targetCalendar: params.targetCalendar,
      startDate: params.startDate,
      endDate: params.endDate,
      syncDirection: params.syncDirection || 'one-way',
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to sync calendars',
    };
  }
}
