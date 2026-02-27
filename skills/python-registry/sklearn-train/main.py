from sklearn.linear_model import LogisticRegression, LinearRegression, Ridge, Lasso
from sklearn.ensemble import RandomForestClassifier, RandomForestRegressor, GradientBoostingClassifier
from sklearn.svm import SVC, SVR
from sklearn.tree import DecisionTreeClassifier, DecisionTreeRegressor
from sklearn.model_selection import train_test_split
import pickle
import base64
from typing import Dict, Any

def execute(context: Dict[str, Any]) -> Dict[str, Any]:
    """
    Train machine learning models using scikit-learn.
    
    Supported models:
    - logistic_regression: Binary/multiclass classification
    - linear_regression: Linear regression
    - random_forest_classifier: Random forest classification
    - random_forest_regressor: Random forest regression
    - svm_classifier: Support vector machine classification
    - decision_tree: Decision tree classifier/regressor
    - gradient_boosting: Gradient boosting classifier
    """
    try:
        params = context.get('parameters', {})
        model_type = params.get('model_type', 'logistic_regression')
        X_train = params.get('X_train')
        y_train = params.get('y_train')
        test_size = params.get('test_size', 0.2)
        random_state = params.get('random_state', 42)
        model_params = params.get('model_params', {})
        
        if X_train is None or y_train is None:
            return {'success': False, 'error': 'X_train and y_train are required'}
        
        # Split data if requested
        if params.get('auto_split', False):
            X_train, X_test, y_train, y_test = train_test_split(
                X_train, y_train, test_size=test_size, random_state=random_state
            )
        
        # Initialize model
        if model_type == 'logistic_regression':
            model = LogisticRegression(**model_params)
        elif model_type == 'linear_regression':
            model = LinearRegression(**model_params)
        elif model_type == 'ridge':
            model = Ridge(**model_params)
        elif model_type == 'lasso':
            model = Lasso(**model_params)
        elif model_type == 'random_forest_classifier':
            model = RandomForestClassifier(**model_params)
        elif model_type == 'random_forest_regressor':
            model = RandomForestRegressor(**model_params)
        elif model_type == 'svm_classifier':
            model = SVC(**model_params)
        elif model_type == 'svm_regressor':
            model = SVR(**model_params)
        elif model_type == 'decision_tree_classifier':
            model = DecisionTreeClassifier(**model_params)
        elif model_type == 'decision_tree_regressor':
            model = DecisionTreeRegressor(**model_params)
        elif model_type == 'gradient_boosting':
            model = GradientBoostingClassifier(**model_params)
        else:
            return {'success': False, 'error': f'Unknown model type: {model_type}'}
        
        # Train model
        model.fit(X_train, y_train)
        
        # Serialize model
        model_bytes = pickle.dumps(model)
        model_b64 = base64.b64encode(model_bytes).decode('utf-8')
        
        # Get training score
        train_score = model.score(X_train, y_train)
        
        result = {
            'model_b64': model_b64,
            'train_score': float(train_score),
            'model_type': model_type,
            'n_samples': len(X_train)
        }
        
        # Add feature importances if available
        if hasattr(model, 'feature_importances_'):
            result['feature_importances'] = model.feature_importances_.tolist()
        
        # Add coefficients if available
        if hasattr(model, 'coef_'):
            result['coefficients'] = model.coef_.tolist()
        
        return {'success': True, 'data': result}
        
    except Exception as e:
        return {'success': False, 'error': str(e)}
