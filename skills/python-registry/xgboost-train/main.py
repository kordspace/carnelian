import xgboost as xgb
import pickle
import base64
from typing import Dict, Any

def execute(context: Dict[str, Any]) -> Dict[str, Any]:
    """
    Train gradient boosting models using XGBoost.
    
    Supports both classification and regression tasks.
    """
    try:
        params = context.get('parameters', {})
        X_train = params.get('X_train')
        y_train = params.get('y_train')
        task = params.get('task', 'classification')
        xgb_params = params.get('xgb_params', {})
        num_boost_round = params.get('num_boost_round', 100)
        
        if X_train is None or y_train is None:
            return {'success': False, 'error': 'X_train and y_train are required'}
        
        # Create DMatrix
        dtrain = xgb.DMatrix(X_train, label=y_train)
        
        # Set default parameters based on task
        if task == 'classification':
            default_params = {
                'objective': 'binary:logistic',
                'eval_metric': 'logloss',
                'max_depth': 6,
                'eta': 0.3
            }
        else:  # regression
            default_params = {
                'objective': 'reg:squarederror',
                'eval_metric': 'rmse',
                'max_depth': 6,
                'eta': 0.3
            }
        
        # Merge with user params
        default_params.update(xgb_params)
        
        # Train model
        model = xgb.train(default_params, dtrain, num_boost_round=num_boost_round)
        
        # Serialize model
        model_bytes = pickle.dumps(model)
        model_b64 = base64.b64encode(model_bytes).decode('utf-8')
        
        # Get feature importance
        importance = model.get_score(importance_type='weight')
        
        return {
            'success': True,
            'data': {
                'model_b64': model_b64,
                'feature_importance': importance,
                'num_boost_round': num_boost_round,
                'task': task
            }
        }
        
    except Exception as e:
        return {'success': False, 'error': str(e)}
