import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface CalctlManageParams {
  command: 'list' | 'add' | 'edit' | 'delete' | 'search';
  calendar?: string;
  event?: string;
  query?: string;
  startDate?: string;
  endDate?: string;
}

export async function execute(
  context: SkillContext,
  params: CalctlManageParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.command) {
    return {
      success: false,
      error: 'command is required',
    };
  }

  try {
    const response = await gateway.call('calctl.manage', {
      command: params.command,
      calendar: params.calendar,
      event: params.event,
      query: params.query,
      startDate: params.startDate,
      endDate: params.endDate,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to execute calctl command',
    };
  }
}
