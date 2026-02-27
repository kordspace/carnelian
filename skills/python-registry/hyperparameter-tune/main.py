from sklearn.model_selection import GridSearchCV, RandomizedSearchCV
from sklearn.linear_model import LogisticRegression, LinearRegression
from sklearn.ensemble import RandomForestClassifier, RandomForestRegressor
import pickle
import base64
from typing import Dict, Any

def execute(context: Dict[str, Any]) -> Dict[str, Any]:
    """
    Hyperparameter optimization for ML models.
    
    Supports:
    - grid_search: Exhaustive grid search
    - random_search: Randomized search
    """
    try:
        params = context.get('parameters', {})
        method = params.get('method', 'grid_search')
        model_type = params.get('model_type', 'logistic_regression')
        X_train = params.get('X_train')
        y_train = params.get('y_train')
        param_grid = params.get('param_grid', {})
        cv = params.get('cv', 5)
        
        if X_train is None or y_train is None:
            return {'success': False, 'error': 'X_train and y_train are required'}
        
        if not param_grid:
            return {'success': False, 'error': 'param_grid is required'}
        
        # Initialize base model
        if model_type == 'logistic_regression':
            base_model = LogisticRegression(max_iter=1000)
        elif model_type == 'linear_regression':
            base_model = LinearRegression()
        elif model_type == 'random_forest_classifier':
            base_model = RandomForestClassifier()
        elif model_type == 'random_forest_regressor':
            base_model = RandomForestRegressor()
        else:
            return {'success': False, 'error': f'Unknown model type: {model_type}'}
        
        # Perform search
        if method == 'grid_search':
            search = GridSearchCV(base_model, param_grid, cv=cv, n_jobs=-1)
        elif method == 'random_search':
            n_iter = params.get('n_iter', 10)
            search = RandomizedSearchCV(base_model, param_grid, n_iter=n_iter, cv=cv, n_jobs=-1)
        else:
            return {'success': False, 'error': f'Unknown method: {method}'}
        
        # Fit
        search.fit(X_train, y_train)
        
        # Serialize best model
        model_bytes = pickle.dumps(search.best_estimator_)
        model_b64 = base64.b64encode(model_bytes).decode('utf-8')
        
        return {
            'success': True,
            'data': {
                'best_params': search.best_params_,
                'best_score': float(search.best_score_),
                'model_b64': model_b64,
                'cv_results': {
                    'mean_test_score': search.cv_results_['mean_test_score'].tolist()[:10],
                    'params': [str(p) for p in search.cv_results_['params'][:10]]
                }
            }
        }
        
    except Exception as e:
        return {'success': False, 'error': str(e)}
