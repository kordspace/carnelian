import pickle
import base64
import numpy as np
from typing import Dict, Any

def execute(context: Dict[str, Any]) -> Dict[str, Any]:
    """
    Make predictions using trained scikit-learn models.
    
    Requires:
    - model_b64: Base64-encoded pickled model
    - X_test: Test data for predictions
    """
    try:
        params = context.get('parameters', {})
        model_b64 = params.get('model_b64')
        X_test = params.get('X_test')
        
        if not model_b64:
            return {'success': False, 'error': 'model_b64 is required'}
        if X_test is None:
            return {'success': False, 'error': 'X_test is required'}
        
        # Deserialize model
        model_bytes = base64.b64decode(model_b64)
        model = pickle.loads(model_bytes)
        
        # Make predictions
        predictions = model.predict(X_test)
        
        result = {
            'predictions': predictions.tolist(),
            'n_samples': len(predictions)
        }
        
        # Add probability predictions if available
        if hasattr(model, 'predict_proba'):
            probabilities = model.predict_proba(X_test)
            result['probabilities'] = probabilities.tolist()
        
        # Add decision function if available
        if hasattr(model, 'decision_function'):
            decision = model.decision_function(X_test)
            result['decision_function'] = decision.tolist()
        
        return {'success': True, 'data': result}
        
    except Exception as e:
        return {'success': False, 'error': str(e)}
