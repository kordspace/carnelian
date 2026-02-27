from sklearn.preprocessing import StandardScaler, MinMaxScaler, LabelEncoder, OneHotEncoder
from sklearn.feature_selection import SelectKBest, f_classif, f_regression
import pandas as pd
import numpy as np
from typing import Dict, Any

def execute(context: Dict[str, Any]) -> Dict[str, Any]:
    """
    Feature transformation and engineering for ML models.
    
    Supported operations:
    - scale: Standardize or normalize features
    - encode: Encode categorical variables
    - select: Feature selection
    - polynomial: Create polynomial features
    - binning: Bin continuous features
    """
    try:
        params = context.get('parameters', {})
        operation = params.get('operation', 'scale')
        data = params.get('data')
        
        if data is None:
            return {'success': False, 'error': 'data is required'}
        
        df = pd.DataFrame(data)
        
        if operation == 'scale':
            method = params.get('method', 'standard')
            columns = params.get('columns', df.select_dtypes(include=[np.number]).columns.tolist())
            
            if method == 'standard':
                scaler = StandardScaler()
            elif method == 'minmax':
                scaler = MinMaxScaler()
            else:
                return {'success': False, 'error': f'Unknown scaling method: {method}'}
            
            df[columns] = scaler.fit_transform(df[columns])
            result_df = df
            
        elif operation == 'encode':
            method = params.get('method', 'onehot')
            columns = params.get('columns', df.select_dtypes(include=['object']).columns.tolist())
            
            if method == 'label':
                for col in columns:
                    le = LabelEncoder()
                    df[col] = le.fit_transform(df[col])
                result_df = df
            elif method == 'onehot':
                result_df = pd.get_dummies(df, columns=columns)
            else:
                return {'success': False, 'error': f'Unknown encoding method: {method}'}
                
        elif operation == 'select':
            k = params.get('k', 10)
            target = params.get('target')
            task = params.get('task', 'classification')
            
            if not target:
                return {'success': False, 'error': 'target column required for feature selection'}
            
            X = df.drop(columns=[target])
            y = df[target]
            
            score_func = f_classif if task == 'classification' else f_regression
            selector = SelectKBest(score_func=score_func, k=min(k, X.shape[1]))
            X_selected = selector.fit_transform(X, y)
            
            selected_features = X.columns[selector.get_support()].tolist()
            result_df = pd.DataFrame(X_selected, columns=selected_features)
            result_df[target] = y.values
            
        elif operation == 'polynomial':
            degree = params.get('degree', 2)
            columns = params.get('columns', df.select_dtypes(include=[np.number]).columns.tolist())
            
            from sklearn.preprocessing import PolynomialFeatures
            poly = PolynomialFeatures(degree=degree, include_bias=False)
            poly_features = poly.fit_transform(df[columns])
            
            feature_names = poly.get_feature_names_out(columns)
            poly_df = pd.DataFrame(poly_features, columns=feature_names)
            
            # Combine with non-polynomial columns
            other_cols = [col for col in df.columns if col not in columns]
            result_df = pd.concat([poly_df, df[other_cols].reset_index(drop=True)], axis=1)
            
        elif operation == 'binning':
            column = params.get('column')
            bins = params.get('bins', 5)
            labels = params.get('labels')
            
            if not column:
                return {'success': False, 'error': 'column required for binning'}
            
            df[f'{column}_binned'] = pd.cut(df[column], bins=bins, labels=labels)
            result_df = df
            
        else:
            return {'success': False, 'error': f'Unknown operation: {operation}'}
        
        return {
            'success': True,
            'data': {
                'result': result_df.to_dict(orient='records'),
                'shape': result_df.shape,
                'columns': result_df.columns.tolist()
            }
        }
        
    except Exception as e:
        return {'success': False, 'error': str(e)}
