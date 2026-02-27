from sklearn.metrics import (
    accuracy_score, precision_score, recall_score, f1_score,
    mean_squared_error, mean_absolute_error, r2_score,
    confusion_matrix, classification_report, roc_auc_score
)
import numpy as np
from typing import Dict, Any

def execute(context: Dict[str, Any]) -> Dict[str, Any]:
    """
    Evaluate ML model performance with comprehensive metrics.
    
    Supports both classification and regression tasks.
    """
    try:
        params = context.get('parameters', {})
        y_true = params.get('y_true')
        y_pred = params.get('y_pred')
        task = params.get('task', 'classification')
        y_prob = params.get('y_prob')  # For classification probabilities
        
        if y_true is None or y_pred is None:
            return {'success': False, 'error': 'y_true and y_pred are required'}
        
        if task == 'classification':
            metrics = {
                'accuracy': float(accuracy_score(y_true, y_pred)),
                'precision': float(precision_score(y_true, y_pred, average='weighted', zero_division=0)),
                'recall': float(recall_score(y_true, y_pred, average='weighted', zero_division=0)),
                'f1_score': float(f1_score(y_true, y_pred, average='weighted', zero_division=0))
            }
            
            # Add confusion matrix
            cm = confusion_matrix(y_true, y_pred)
            metrics['confusion_matrix'] = cm.tolist()
            
            # Add ROC AUC if probabilities provided
            if y_prob is not None:
                try:
                    metrics['roc_auc'] = float(roc_auc_score(y_true, y_prob, multi_class='ovr'))
                except:
                    pass
            
            # Add classification report
            report = classification_report(y_true, y_pred, output_dict=True, zero_division=0)
            metrics['classification_report'] = report
            
        else:  # regression
            metrics = {
                'mse': float(mean_squared_error(y_true, y_pred)),
                'rmse': float(np.sqrt(mean_squared_error(y_true, y_pred))),
                'mae': float(mean_absolute_error(y_true, y_pred)),
                'r2_score': float(r2_score(y_true, y_pred))
            }
            
            # Add additional regression metrics
            residuals = np.array(y_true) - np.array(y_pred)
            metrics['mean_residual'] = float(np.mean(residuals))
            metrics['std_residual'] = float(np.std(residuals))
        
        return {
            'success': True,
            'data': {
                'task': task,
                'metrics': metrics,
                'n_samples': len(y_true)
            }
        }
        
    except Exception as e:
        return {'success': False, 'error': str(e)}
