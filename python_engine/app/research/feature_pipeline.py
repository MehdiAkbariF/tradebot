# مسیر: python_engine/app/research/feature_pipeline.py
import os
from datetime import datetime, timezone, timedelta
import numpy as np
import pandas as pd
import lightgbm as lgb
from loguru import logger
from sklearn.model_selection import train_test_split
from sklearn.metrics import roc_auc_score

FEATURE_SCHEMA_VERSION = "2.0.0"
STRATEGY_VERSION = "wave_breakout_v1"

class ScalpMultiTimeframePipeline:
    def __init__(self, tp_bps: float = 14.0, sl_bps: float = 14.0, time_barrier_sec: int = 60):
        self.tp_bps = tp_bps / 10000.0
        self.sl_bps = sl_bps / 10000.0
        self.time_barrier_sec = time_barrier_sec
        self.decay_lambda = 0.0069

    def build_multitimeframe_features(self, df: pd.DataFrame) -> pd.DataFrame:
        logger.info("Extracting Canonical Microstructure features (Strict Validation)...")
        df = df.sort_values("timestamp").reset_index(drop=True).copy()

        # ⚠️ گارد اعتبارسنجی: اگر ستون OFI نباشد، برنامه به جای تولید نویز تصادفی باید خطا دهد
        if "ofi" not in df.columns:
            logger.critical("Integrity Error: 'ofi' column is missing from raw dataset. Refusing to inject random noise.")
            raise ValueError("Data Integrity Violation: Missing OFI in training pipeline.")

        df["log_ret_1s"] = np.log(df["price"] / df["price"].shift(1)).fillna(0)
        df["log_ret_5s"] = np.log(df["price"] / df["price"].shift(5)).fillna(0)
        df["log_ret_15s"] = np.log(df["price"] / df["price"].shift(15)).fillna(0)
        df["realized_vol_30s"] = df["log_ret_1s"].rolling(30).std().fillna(0)

        df["ema_fast_15m"] = df["price"].ewm(span=900, adjust=False).mean()
        df["ema_slow_15m"] = df["price"].ewm(span=3600, adjust=False).mean()
        df["trend_15m_bias"] = np.where(df["ema_fast_15m"] > df["ema_slow_15m"], 1.0, -1.0)

        df["cum_vol"] = df["volume"].cumsum()
        df["cum_vol_price"] = (df["price"] * df["volume"]).cumsum()
        df["daily_vwap"] = df["cum_vol_price"] / df["cum_vol"]
        df["dist_to_vwap_bps"] = ((df["price"] - df["daily_vwap"]) / df["daily_vwap"]) * 10000.0
        df["volume_surge"] = df["volume"] / df["volume"].rolling(60).mean().fillna(1)

        if "decayed_sentiment" not in df.columns:
            df["decayed_sentiment"] = 0.0

        return df

    def apply_triple_barrier_labels(self, df: pd.DataFrame) -> pd.DataFrame:
        logger.info("Applying Strict Triple-Barrier labeling...")
        prices = df["price"].values
        n = len(prices)
        labels = np.zeros(n, dtype=int)

        for i in range(n - self.time_barrier_sec):
            entry_price = prices[i]
            upper_barrier = entry_price * (1.0 + self.tp_bps)
            lower_barrier = entry_price * (1.0 - self.sl_bps)

            window_prices = prices[i + 1 : i + self.time_barrier_sec + 1]
            hit_tp = np.where(window_prices >= upper_barrier)[0]
            hit_sl = np.where(window_prices <= lower_barrier)[0]

            first_tp = hit_tp[0] if len(hit_tp) > 0 else 999999
            first_sl = hit_sl[0] if len(hit_sl) > 0 else 999999

            if first_tp < first_sl:
                labels[i] = 1  # Long Hit Target
            elif first_sl < first_tp:
                labels[i] = 2  # Short Hit Target
            else:
                labels[i] = 0  # Timeout / Choppy

        df["target"] = labels
        return df.iloc[:-self.time_barrier_sec].reset_index(drop=True)

    def train_production_model(self, df: pd.DataFrame, model_output_path: str = "scalp_lightgbm_model.txt"):
        df_featured = self.build_multitimeframe_features(df)
        dataset = self.apply_triple_barrier_labels(df_featured)

        feature_cols = [
            "log_ret_1s", "log_ret_5s", "log_ret_15s",
            "realized_vol_30s", "trend_15m_bias",
            "dist_to_vwap_bps", "volume_surge", "ofi",
            "decayed_sentiment"
        ]

        X = dataset[feature_cols]
        y = np.where(dataset["target"] == 1, 1, 0)

        # تقسیم زمانی اکید بدون Shuffle
        X_train, X_test, y_train, y_test = train_test_split(X, y, test_size=0.2, shuffle=False)

        params = {
            "objective": "binary",
            "metric": ["auc", "binary_logloss"],
            "boosting_type": "gbdt",
            "learning_rate": 0.02,
            "num_leaves": 15,
            "max_depth": 4,
            "min_data_in_leaf": 30,
            "feature_fraction": 0.8,
            "verbose": -1,
            "n_jobs": -1
        }

        train_data = lgb.Dataset(X_train, label=y_train)
        valid_data = lgb.Dataset(X_test, label=y_test, reference=train_data)

        logger.info("Training Production Model with Walk-Forward Split...")
        model = lgb.train(params, train_data, num_boost_round=300, valid_sets=[valid_data])

        test_preds = model.predict(X_test, num_iteration=model.best_iteration)
        if len(np.unique(y_test)) > 1:
            auc = roc_auc_score(y_test, test_preds)
            logger.info(f"Model AUC Verified: {auc:.4f}")
        
        model.save_model(model_output_path)
        logger.success(f"Production Model Saved: {model_output_path}")