import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface CronAddParams {
  name?: string;
  schedule: {
    kind: 'at' | 'every' | 'cron';
    atMs?: number;
    everyMs?: number;
    anchorMs?: number;
    expr?: string;
    tz?: string;
  };
  payload: {
    kind: 'systemEvent' | 'agentTurn';
    text?: string;
    message?: string;
    model?: string;
    thinking?: string;
    timeoutSeconds?: number;
    deliver?: boolean;
    channel?: string;
    to?: string;
    bestEffortDeliver?: boolean;
  };
  sessionTarget: 'main' | 'isolated';
  enabled?: boolean;
  agentId?: string;
}

export async function execute(
  context: SkillContext,
  params: CronAddParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.schedule || !params.payload || !params.sessionTarget) {
    return {
      success: false,
      error: 'schedule, payload, and sessionTarget are required',
    };
  }

  if (params.sessionTarget === 'main' && params.payload.kind !== 'systemEvent') {
    return {
      success: false,
      error: 'sessionTarget="main" requires payload.kind="systemEvent"',
    };
  }

  if (params.sessionTarget === 'isolated' && params.payload.kind !== 'agentTurn') {
    return {
      success: false,
      error: 'sessionTarget="isolated" requires payload.kind="agentTurn"',
    };
  }

  try {
    const job = {
      name: params.name,
      schedule: params.schedule,
      payload: params.payload,
      sessionTarget: params.sessionTarget,
      enabled: params.enabled ?? true,
      agentId: params.agentId,
    };

    const response = await gateway.call('cron.add', job);

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to add cron job',
    };
  }
}
