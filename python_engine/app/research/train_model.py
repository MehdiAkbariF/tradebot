# مسیر: python_engine/app/research/train_model.py
import numpy as np
import pandas as pd
import lightgbm as lgb
from loguru import logger

def generate_initial_microstructure_model():
    logger.info("Training Canonical Microstructure LightGBM Booster...")

    # ساخت داده‌های تیک بر پایه توزیع نرمال میکرواستراکچر واقعی
    np.random.seed(42)
    n_samples = 50000
    
    ofi = np.random.uniform(-1.0, 1.0, n_samples)
    vol_bps = np.random.exponential(scale=2.5, size=n_samples) + 0.5
    volume = np.random.exponential(scale=1.5, size=n_samples) + 0.1

    # هدف چندکلاسه بر پایه تعادل جریان سفارشات:
    # 0 = Timeout, 1 = Take-Profit, 2 = Stop-Loss
    target = np.zeros(n_samples, dtype=int)
    for i in range(n_samples):
        if ofi[i] > 0.45 and vol_bps[i] > 1.2:
            target[i] = 1 # TP
        elif ofi[i] < -0.45 and vol_bps[i] > 1.2:
            target[i] = 2 # SL
        else:
            target[i] = 0 # Timeout

    X = pd.DataFrame({"ofi": ofi, "vol_bps": vol_bps, "volume": volume})
    y = target

    train_data = lgb.Dataset(X, label=y)
    params = {
        "objective": "multiclass",
        "num_class": 3,
        "metric": "multi_logloss",
        "learning_rate": 0.05,
        "num_leaves": 15,
        "max_depth": 4,
        "verbose": -1
    }

    model = lgb.train(params, train_data, num_boost_round=100)
    model.save_model("scalp_lightgbm_model.txt")
    logger.success("✅ Clean Base Model saved as 'scalp_lightgbm_model.txt'")

if __name__ == "__main__":
    generate_initial_microstructure_model()