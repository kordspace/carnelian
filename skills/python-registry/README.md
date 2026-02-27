# Python Skills Registry

This directory contains Python-based skills for data science, machine learning, and analytics workloads.

## Structure

Each skill follows this structure:
```
skill-name/
├── skill.json       # Skill metadata
├── main.py          # Skill implementation
└── requirements.txt # Python dependencies (optional)
```

## Skill Implementation

Python skills must implement an `execute` function:

```python
def execute(context: dict) -> dict:
    """
    Execute the skill with the given context.
    
    Args:
        context: Dictionary containing:
            - parameters: Skill-specific parameters
            - gateway_url: Gateway URL for API calls
            
    Returns:
        Dictionary with:
            - success: Boolean indicating success
            - data: Result data (if successful)
            - error: Error message (if failed)
    """
    pass
```

## Available Skills

### Data Processing
- `pandas-dataframe` - DataFrame operations (filter, group, merge)
- `numpy-array` - Array computations and linear algebra
- `polars-query` - Fast DataFrame queries

### Visualization
- `matplotlib-plot` - Static plots (line, bar, scatter)
- `seaborn-visualize` - Statistical visualizations
- `plotly-interactive` - Interactive charts

### Machine Learning
- `sklearn-train` - Train ML models (classification, regression)
- `sklearn-predict` - Model inference
- `xgboost-train` - Gradient boosting

### Statistical Analysis
- `scipy-stats` - Statistical tests
- `statsmodels-regression` - Statistical modeling

## Dependencies

Core dependencies are managed in `workers/python-worker/requirements.txt`. Skill-specific dependencies can be added in individual skill directories.
