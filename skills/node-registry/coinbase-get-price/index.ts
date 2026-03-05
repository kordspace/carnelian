import type { SkillContext, SkillResult } from '../../types';

interface CoinbaseGetPriceParams {
  currencyPair: string;
}

export async function execute(
  context: SkillContext,
  params: CoinbaseGetPriceParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.currencyPair) {
    return {
      success: false,
      error: 'currencyPair is required',
    };
  }

  try {
    const response = await gateway.call('coinbase.getPrice', {
      currencyPair: params.currencyPair,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to get Coinbase price',
    };
  }
}
