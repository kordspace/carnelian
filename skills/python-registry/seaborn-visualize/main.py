import seaborn as sns
import matplotlib.pyplot as plt
import matplotlib
matplotlib.use('Agg')
import pandas as pd
import base64
from io import BytesIO
from typing import Dict, Any

def execute(context: Dict[str, Any]) -> Dict[str, Any]:
    """
    Create statistical visualizations using Seaborn.
    
    Supported plot types:
    - heatmap: Correlation heatmap
    - pairplot: Pairwise relationships
    - boxplot: Box plot with categories
    - violinplot: Violin plot
    - distplot: Distribution plot
    - regplot: Regression plot
    - countplot: Count plot for categorical data
    """
    try:
        params = context.get('parameters', {})
        plot_type = params.get('plot_type', 'heatmap')
        data = params.get('data')
        figsize = params.get('figsize', (10, 8))
        
        if data is None:
            return {'success': False, 'error': 'data is required'}
        
        # Convert to DataFrame
        df = pd.DataFrame(data)
        
        # Create figure
        plt.figure(figsize=figsize)
        
        if plot_type == 'heatmap':
            corr = df.corr() if params.get('correlation', True) else df
            sns.heatmap(corr, annot=params.get('annot', True), cmap=params.get('cmap', 'coolwarm'))
            
        elif plot_type == 'pairplot':
            g = sns.pairplot(df, hue=params.get('hue'))
            plt.close()
            
        elif plot_type == 'boxplot':
            x = params.get('x')
            y = params.get('y')
            if not y:
                return {'success': False, 'error': 'y parameter required for boxplot'}
            sns.boxplot(data=df, x=x, y=y, hue=params.get('hue'))
            
        elif plot_type == 'violinplot':
            x = params.get('x')
            y = params.get('y')
            if not y:
                return {'success': False, 'error': 'y parameter required for violinplot'}
            sns.violinplot(data=df, x=x, y=y, hue=params.get('hue'))
            
        elif plot_type == 'distplot':
            column = params.get('column')
            if not column:
                return {'success': False, 'error': 'column parameter required for distplot'}
            sns.histplot(df[column], kde=params.get('kde', True))
            
        elif plot_type == 'regplot':
            x = params.get('x')
            y = params.get('y')
            if not x or not y:
                return {'success': False, 'error': 'x and y parameters required for regplot'}
            sns.regplot(data=df, x=x, y=y)
            
        elif plot_type == 'countplot':
            x = params.get('x')
            if not x:
                return {'success': False, 'error': 'x parameter required for countplot'}
            sns.countplot(data=df, x=x, hue=params.get('hue'))
            
        else:
            return {'success': False, 'error': f'Unknown plot type: {plot_type}'}
        
        # Set title if provided
        if params.get('title'):
            plt.title(params['title'])
        
        # Save to base64
        buffer = BytesIO()
        plt.savefig(buffer, format='png', bbox_inches='tight', dpi=params.get('dpi', 100))
        buffer.seek(0)
        image_b64 = base64.b64encode(buffer.read()).decode('utf-8')
        plt.close('all')
        
        return {
            'success': True,
            'data': {
                'image_b64': image_b64,
                'plot_type': plot_type,
                'format': 'png'
            }
        }
        
    except Exception as e:
        return {'success': False, 'error': str(e)}
