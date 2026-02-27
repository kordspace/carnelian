import statsmodels.api as sm
import pandas as pd
from typing import Dict, Any

def execute(context: Dict[str, Any]) -> Dict[str, Any]:
    """
    Statistical regression modeling using statsmodels.
    
    Supports:
    - OLS: Ordinary Least Squares
    - Logit: Logistic Regression
    - GLM: Generalized Linear Models
    """
    try:
        params = context.get('parameters', {})
        model_type = params.get('model_type', 'OLS')
        X = params.get('X')
        y = params.get('y')
        add_constant = params.get('add_constant', True)
        
        if X is None or y is None:
            return {'success': False, 'error': 'X and y are required'}
        
        # Convert to DataFrame if needed
        if isinstance(X, list):
            X = pd.DataFrame(X)
        
        # Add constant term if requested
        if add_constant:
            X = sm.add_constant(X)
        
        # Fit model based on type
        if model_type == 'OLS':
            model = sm.OLS(y, X)
        elif model_type == 'Logit':
            model = sm.Logit(y, X)
        elif model_type == 'GLM':
            family = params.get('family', 'gaussian')
            if family == 'gaussian':
                fam = sm.families.Gaussian()
            elif family == 'binomial':
                fam = sm.families.Binomial()
            elif family == 'poisson':
                fam = sm.families.Poisson()
            else:
                return {'success': False, 'error': f'Unknown family: {family}'}
            model = sm.GLM(y, X, family=fam)
        else:
            return {'success': False, 'error': f'Unknown model type: {model_type}'}
        
        # Fit the model
        results = model.fit()
        
        # Extract results
        return {
            'success': True,
            'data': {
                'model_type': model_type,
                'params': results.params.to_dict(),
                'pvalues': results.pvalues.to_dict(),
                'rsquared': float(results.rsquared) if hasattr(results, 'rsquared') else None,
                'rsquared_adj': float(results.rsquared_adj) if hasattr(results, 'rsquared_adj') else None,
                'aic': float(results.aic),
                'bic': float(results.bic),
                'summary': str(results.summary())
            }
        }
        
    except Exception as e:
        return {'success': False, 'error': str(e)}
