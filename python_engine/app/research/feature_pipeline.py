import numpy as np
import pandas as pd
import lightgbm as lgb
from sklearn.isotonic import IsotonicRegression
from sklearn.metrics import log_loss, brier_score_loss
from loguru import logger
import os

class CanonicalPurgedResearchPipeline:
    def __init__(self, tp_bps: float = 6.0, sl_bps: float = 12.0, time_barrier_sec: int = 15):
        self.tp_threshold = tp_bps / 10000.0
        self.sl_threshold = sl_bps / 10000.0
        self.time_barrier_sec = time_barrier_sec
        self.feature_columns = [
            "log_ret_1s", "log_ret_5s", "ofi_raw", "ofi_rolling_30s",
            "spread_bps", "realized_vol_60s", "book_imbalance"
        ]

    def extract_causal_features(self, raw_ticks_df: pd.DataFrame) -> pd.DataFrame:
        """Construct features using only historical causal information."""
        df = raw_ticks_df.copy()
        df["timestamp"] = pd.to_datetime(df["timestamp"])
        df = df.sort_values("timestamp").reset_index(drop=True)

        df["log_ret_1s"] = np.log(df["price"] / df["price"].shift(1)).fillna(0.0)
        df["log_ret_5s"] = np.log(df["price"] / df["price"].shift(5)).fillna(0.0)
        df["ofi_raw"] = df["ofi"].fillna(0.0)
        df["ofi_rolling_30s"] = df["ofi_raw"].rolling(30, min_periods=1).mean()
        df["spread_bps"] = ((df["ask"] - df["bid"]) / df["price"]).fillna(0.0) * 10000.0
        df["realized_vol_60s"] = df["log_ret_1s"].rolling(60, min_periods=5).std().fillna(0.0001) * 10000.0
        df["book_imbalance"] = df["book_imbalance"].fillna(0.0)

        return df.dropna().reset_index(drop=True)

    def apply_triple_barrier_labeling(self, df: pd.DataFrame) -> pd.DataFrame:
        """
        Label outcomes strictly:
        0: TIMEOUT (Unrealized decay)
        1: TAKE_PROFIT (Upper barrier hit first)
        2: STOP_LOSS (Lower barrier hit first)
        """
        timestamps = df["timestamp"].values
        prices = df["price"].values
        n = len(df)
        labels = np.zeros(n, dtype=int)
        time_limit = np.timedelta64(self.time_barrier_sec, "s")

        for i in range(n):
            t0 = timestamps[i]
            p0 = prices[i]
            upper_bound = p0 * (1.0 + self.tp_threshold)
            lower_bound = p0 * (1.0 - self.sl_threshold)

            j = i + 1
            assigned_label = 0
            while j < n and (timestamps[j] - t0) <= time_limit:
                p_curr = prices[j]
                if p_curr >= upper_bound:
                    assigned_label = 1
                    break
                elif p_curr <= lower_bound:
                    assigned_label = 2
                    break
                j += 1
            labels[i] = assigned_label

        df["target"] = labels
        return df

    def run_purged_walk_forward_validation(self, df: pd.DataFrame, n_splits: int = 5):
        """Walk-Forward validation with an embargo window between train and test splits."""
        logger.info(f"Executing Purged Walk-Forward Cross-Validation ({n_splits} folds)...")
        labeled_df = self.apply_triple_barrier_labeling(self.extract_causal_features(df))
        
        split_size = len(labeled_df) // (n_splits + 1)
        embargo = int(self.time_barrier_sec * 2) # Buffer to prevent overlapping horizon labels

        fold_scores = []
        for fold in range(1, n_splits + 1):
            train_end = fold * split_size
            test_start = train_end + embargo
            test_end = test_start + split_size

            if test_end > len(labeled_df):
                break

            train_data = labeled_df.iloc[:train_end]
            test_data = labeled_df.iloc[test_start:test_end]

            X_train, y_train = train_data[self.feature_columns], train_data["target"]
            X_test, y_test = test_data[self.feature_columns], test_data["target"]

            train_ds = lgb.Dataset(X_train, label=y_train)
            val_ds = lgb.Dataset(X_test, label=y_test, reference=train_ds)

            params = {
                "objective": "multiclass",
                "num_class": 3,
                "metric": "multi_logloss",
                "boosting": "gbdt",
                "learning_rate": 0.03,
                "num_leaves": 15,
                "max_depth": 4,
                "feature_fraction": 0.8,
                "verbose": -1
            }

            booster = lgb.train(params, train_ds, num_boost_round=150, valid_sets=[val_ds])
            raw_preds = booster.predict(X_test)
            loss = log_loss(y_test, raw_preds)
            fold_scores.append(loss)
            logger.info(f"Fold {fold} Multi-LogLoss: {loss:.4f}")

        logger.success(f"Mean OOS LogLoss across folds: {np.mean(fold_scores):.4f}")