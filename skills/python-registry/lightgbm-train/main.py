import lightgbm as lgb
import pickle
import base64
from typing import Dict, Any

def execute(context: Dict[str, Any]) -> Dict[str, Any]:
    """
    Train fast gradient boosting models using LightGBM.
    
    Supports both classification and regression tasks.
    """
    try:
        params = context.get('parameters', {})
        X_train = params.get('X_train')
        y_train = params.get('y_train')
        task = params.get('task', 'classification')
        lgb_params = params.get('lgb_params', {})
        num_boost_round = params.get('num_boost_round', 100)
        
        if X_train is None or y_train is None:
            return {'success': False, 'error': 'X_train and y_train are required'}
        
        # Create dataset
        train_data = lgb.Dataset(X_train, label=y_train)
        
        # Set default parameters based on task
        if task == 'classification':
            default_params = {
                'objective': 'binary',
                'metric': 'binary_logloss',
                'boosting_type': 'gbdt',
                'num_leaves': 31,
                'learning_rate': 0.05
            }
        else:  # regression
            default_params = {
                'objective': 'regression',
                'metric': 'rmse',
                'boosting_type': 'gbdt',
                'num_leaves': 31,
                'learning_rate': 0.05
            }
        
        # Merge with user params
        default_params.update(lgb_params)
        
        # Train model
        model = lgb.train(default_params, train_data, num_boost_round=num_boost_round)
        
        # Serialize model
        model_bytes = pickle.dumps(model)
        model_b64 = base64.b64encode(model_bytes).decode('utf-8')
        
        # Get feature importance
        importance = dict(zip(range(model.num_feature()), model.feature_importance().tolist()))
        
        return {
            'success': True,
            'data': {
                'model_b64': model_b64,
                'feature_importance': importance,
                'num_boost_round': num_boost_round,
                'task': task,
                'num_features': model.num_feature()
            }
        }
        
    except Exception as e:
        return {'success': False, 'error': str(e)}
