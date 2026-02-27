from scipy import stats
import numpy as np
from typing import Dict, Any

def execute(context: Dict[str, Any]) -> Dict[str, Any]:
    """
    Perform statistical tests and operations using SciPy.
    
    Supported tests:
    - ttest: T-test (independent or paired)
    - anova: One-way ANOVA
    - chi2: Chi-square test
    - correlation: Pearson/Spearman correlation
    - normality: Normality tests (Shapiro-Wilk, Kolmogorov-Smirnov)
    - distribution: Fit distribution and get statistics
    """
    try:
        params = context.get('parameters', {})
        test_type = params.get('test_type', 'ttest')
        data1 = params.get('data1')
        data2 = params.get('data2')
        
        if test_type == 'ttest':
            if data1 is None or data2 is None:
                return {'success': False, 'error': 'data1 and data2 required for t-test'}
            paired = params.get('paired', False)
            if paired:
                statistic, pvalue = stats.ttest_rel(data1, data2)
            else:
                statistic, pvalue = stats.ttest_ind(data1, data2)
            return {
                'success': True,
                'data': {
                    'test': 'ttest',
                    'statistic': float(statistic),
                    'pvalue': float(pvalue),
                    'paired': paired
                }
            }
            
        elif test_type == 'anova':
            groups = params.get('groups')
            if not groups or len(groups) < 2:
                return {'success': False, 'error': 'At least 2 groups required for ANOVA'}
            statistic, pvalue = stats.f_oneway(*groups)
            return {
                'success': True,
                'data': {
                    'test': 'anova',
                    'statistic': float(statistic),
                    'pvalue': float(pvalue),
                    'n_groups': len(groups)
                }
            }
            
        elif test_type == 'chi2':
            observed = params.get('observed')
            expected = params.get('expected')
            if observed is None:
                return {'success': False, 'error': 'observed frequencies required'}
            if expected is None:
                statistic, pvalue = stats.chisquare(observed)
            else:
                statistic, pvalue = stats.chisquare(observed, expected)
            return {
                'success': True,
                'data': {
                    'test': 'chi2',
                    'statistic': float(statistic),
                    'pvalue': float(pvalue)
                }
            }
            
        elif test_type == 'correlation':
            if data1 is None or data2 is None:
                return {'success': False, 'error': 'data1 and data2 required for correlation'}
            method = params.get('method', 'pearson')
            if method == 'pearson':
                corr, pvalue = stats.pearsonr(data1, data2)
            elif method == 'spearman':
                corr, pvalue = stats.spearmanr(data1, data2)
            else:
                return {'success': False, 'error': f'Unknown correlation method: {method}'}
            return {
                'success': True,
                'data': {
                    'test': 'correlation',
                    'method': method,
                    'correlation': float(corr),
                    'pvalue': float(pvalue)
                }
            }
            
        elif test_type == 'normality':
            if data1 is None:
                return {'success': False, 'error': 'data1 required for normality test'}
            method = params.get('method', 'shapiro')
            if method == 'shapiro':
                statistic, pvalue = stats.shapiro(data1)
            elif method == 'kstest':
                statistic, pvalue = stats.kstest(data1, 'norm')
            else:
                return {'success': False, 'error': f'Unknown normality test: {method}'}
            return {
                'success': True,
                'data': {
                    'test': 'normality',
                    'method': method,
                    'statistic': float(statistic),
                    'pvalue': float(pvalue),
                    'is_normal': pvalue > 0.05
                }
            }
            
        elif test_type == 'distribution':
            if data1 is None:
                return {'success': False, 'error': 'data1 required for distribution'}
            return {
                'success': True,
                'data': {
                    'mean': float(np.mean(data1)),
                    'median': float(np.median(data1)),
                    'std': float(np.std(data1)),
                    'var': float(np.var(data1)),
                    'skewness': float(stats.skew(data1)),
                    'kurtosis': float(stats.kurtosis(data1)),
                    'min': float(np.min(data1)),
                    'max': float(np.max(data1))
                }
            }
            
        else:
            return {'success': False, 'error': f'Unknown test type: {test_type}'}
        
    except Exception as e:
        return {'success': False, 'error': str(e)}
