import pandas as pd
import numpy as np
from loguru import logger
import lightgbm as lgb
from sklearn.model_selection import train_test_split
from sklearn.metrics import accuracy_score, precision_score, recall_score
from feature_engineering import ScalpFeatureEngineering

def train_scalping_model():
    logger.info("Initializing Model Training Pipeline (LightGBM)...")

    # 1. تولید یا بارگذاری دیتاست تاریخی (در اینجا از داده‌های تستی شبیه‌سازی شده استفاده می‌کنیم)
    dates = pd.date_range(start="2026-01-01", periods=10000, freq="s")
    mock_ticks = pd.DataFrame({
        "timestamp": dates,
        "price": 65000.0 + np.cumsum(np.random.randn(10000) * 1.5),
        "volume": np.random.uniform(0.1, 5.0, 10000),
        "ofi": np.random.uniform(-1, 1, 10000)
    })

    fe = ScalpFeatureEngineering(horizon_seconds=10)
    df = fe.generate_features_and_labels(mock_ticks, pd.DataFrame())

    if df.empty:
        logger.error("Dataset is empty. Cannot train model.")
        return

    # انتخاب فیچرها و هدف (Features & Target)
    feature_cols = ["returns_1s", "returns_10s", "rolling_volatility_30s", "volume_ma_30s", "ofi"] if "ofi" in df.columns else ["returns_1s", "returns_10s", "rolling_volatility_30s", "volume_ma_30s"]
    
    X = df[feature_cols]
    y = df["target"]

    # تقسیم داده به Train و Test با رعایت ترتیب زمانی (Walk-Forward Split)
    X_train, X_test, y_train, y_test = train_test_split(X, y, test_size=0.2, shuffle=False)

    # تنظیمات مدل LightGBM
    params = {
        "objective": "binary",
        "metric": "binary_logloss",
        "boosting_type": "gbdt",
        "learning_rate": 0.05,
        "num_leaves": 31,
        "verbose": -1
    }

    train_data = lgb.Dataset(X_train, label=y_train)
    valid_data = lgb.Dataset(X_test, label=y_test, reference=train_data)

    logger.info("Training LightGBM model...")
    model = lgb.train(
        params,
        train_data,
        num_boost_round=200,
        valid_sets=[valid_data],
    )

    # ارزیابی مدل
    preds_prob = model.predict(X_test, num_iteration=model.best_iteration)
    preds = np.where(preds_prob > 0.55, 1, 0) # آستانه سخت‌گیرانه‌تر برای دقت بالاتر در اسکلپ

    acc = accuracy_score(y_test, preds)
    precision = precision_score(y_test, preds, zero_division=0)
    recall = recall_score(y_test, preds, zero_division=0)

    logger.info(f"Model Training Complete! Metrics -> Accuracy: {acc:.4f} | Precision: {precision:.4f} | Recall: {recall:.4f}")

    # ذخیره مدل آموزش‌دیده
    model.save_model("scalp_lightgbm_model.txt")
    logger.info("Trained model saved successfully as 'scalp_lightgbm_model.txt'.")

if __name__ == "__main__":
    train_scalping_model()