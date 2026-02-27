import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface KrakenTickerParams {
  pair: string;
}

export async function execute(
  context: SkillContext,
  params: KrakenTickerParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.pair) {
    return {
      success: false,
      error: 'pair is required',
    };
  }

  try {
    const response = await gateway.call('kraken.ticker', {
      pair: params.pair,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to fetch Kraken ticker',
    };
  }
}
