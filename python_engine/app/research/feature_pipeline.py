import numpy as np
import pandas as pd
import lightgbm as lgb
from datetime import datetime, timezone, timedelta
from loguru import logger
from sklearn.model_selection import train_test_split
from sklearn.metrics import classification_report, roc_auc_score
import os

class ScalpMultiTimeframePipeline:
    def __init__(self, tp_bps: float = 20.0, sl_bps: float = 12.0, time_barrier_sec: int = 45):
        """
        tp_bps: حد سود بر حسب Basis Points (20 bps = 0.20%)
        sl_bps: حد ضرر بر حسب Basis Points (12 bps = 0.12%)
        time_barrier_sec: حداکثر زمان باز ماندن پوزیشن اسکلپ (45 ثانیه)
        """
        self.tp_bps = tp_bps / 10000.0
        self.sl_bps = sl_bps / 10000.0
        self.time_barrier_sec = time_barrier_sec
        self.decay_lambda = 0.0069  # نیمه‌عمر خبر حدود ۱۰۰ ثانیه

    def compute_decayed_sentiment(self, news_events: list[dict], current_ts: datetime) -> float:
        """محاسبه سنتیمنت تجمعی و تنزیل‌یافته اخبار در لحظه فعلی"""
        if not news_events:
            return 0.0
        total_sentiment = 0.0
        for event in news_events:
            event_ts = datetime.fromisoformat(event["timestamp"])
            delta_sec = (current_ts - event_ts).total_seconds()
            if 0 <= delta_sec <= 600:  # اخبار تا ۱۰ دقیقه قبل معتبرند
                decay_factor = np.exp(-self.decay_lambda * delta_sec)
                score = event.get("sentiment_score", 0.0) * event.get("relevance", 1.0)
                total_sentiment += score * decay_factor
        return float(np.clip(total_sentiment, -1.0, 1.0))

    def build_multitimeframe_features(self, df: pd.DataFrame) -> pd.DataFrame:
        """استخراج فیچرهای روند گذشته، سطوح نقدینگی و مومنتوم ریزساختار"""
        logger.info("Extracting Multi-Timeframe & Microstructure features...")
        df = df.sort_values("timestamp").reset_index(drop=True).copy()

        # ۱. بازده‌های لگاریتمی ریزساختار (۱ ثانیه، ۵ ثانیه، ۱۵ ثانیه)
        df["log_ret_1s"] = np.log(df["price"] / df["price"].shift(1)).fillna(0)
        df["log_ret_5s"] = np.log(df["price"] / df["price"].shift(5)).fillna(0)
        df["log_ret_15s"] = np.log(df["price"] / df["price"].shift(15)).fillna(0)

        # ۲. نوسان تاریخی لحظه‌ای (Realized Volatility)
        df["realized_vol_30s"] = df["log_ret_1s"].rolling(30).std().fillna(0)

        # ۳. محاسبه روند تایم‌فریم بالاتر (۱۵ دقیقه‌ای و ۱ ساعته)
        df["ema_fast_15m"] = df["price"].ewm(span=900, adjust=False).mean()
        df["ema_slow_15m"] = df["price"].ewm(span=3600, adjust=False).mean()
        df["trend_15m_bias"] = np.where(df["ema_fast_15m"] > df["ema_slow_15m"], 1.0, -1.0)

        # ۴. فاصله قیمت از VWAP روزانه
        df["cum_vol"] = df["volume"].cumsum()
        df["cum_vol_price"] = (df["price"] * df["volume"]).cumsum()
        df["daily_vwap"] = df["cum_vol_price"] / df["cum_vol"]
        df["dist_to_vwap_bps"] = ((df["price"] - df["daily_vwap"]) / df["daily_vwap"]) * 10000.0

        # ۵. فیچرهای جریان سفارشات (OFI و عدم تعادل حجم)
        df["volume_surge"] = df["volume"] / df["volume"].rolling(60).mean().fillna(1)
        if "ofi" not in df.columns:
            df["ofi"] = np.random.uniform(-0.5, 0.5, len(df))

        # ۶. فیچر سنتیمنت تنزیل یافته
        if "decayed_sentiment" not in df.columns:
            df["decayed_sentiment"] = 0.0

        return df

    def apply_triple_barrier_labels(self, df: pd.DataFrame) -> pd.DataFrame:
        """برچسب‌گذاری دقیق بر مبنای تاچ شدن حد سود قبل از حد ضرر در بازه ۴۵ ثانیه"""
        logger.info("Applying Triple-Barrier labeling (TP/SL/Time-out)...")
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
                labels[i] = 1  # ستاپ موفق خرید (BUY)
            elif first_sl < first_tp:
                labels[i] = 2  # ستاپ موفق فروش (SELL)
            else:
                labels[i] = 0  # بدون معامله یا خروج با انقضای زمان

        df["target"] = labels
        # حذف داده‌های انتهای دیتاست به دلیل عدم تکمیل پنجره زمانی
        return df.iloc[:-self.time_barrier_sec].reset_index(drop=True)

    def train_production_model(self, df: pd.DataFrame, model_output_path: str = "scalp_lightgbm_model.txt"):
        """آموزش مدل LightGBM طبق متدولوژی واک-فوروارد (Walk-Forward)"""
        df_featured = self.build_multitimeframe_features(df)
        dataset = self.apply_triple_barrier_labels(df_featured)

        feature_cols = [
            "log_ret_1s", "log_ret_5s", "log_ret_15s",
            "realized_vol_30s", "trend_15m_bias",
            "dist_to_vwap_bps", "volume_surge", "ofi",
            "decayed_sentiment"
        ]

        X = dataset[feature_cols]
        # تبدیل تارگت به باینری برای سیگنال لانگ (۱ = لانگ، ۰ = غیر از آن)
        y = np.where(dataset["target"] == 1, 1, 0)

        # تقسیم داده‌ها بدون Shuffle برای حفظ ترتیب زمانی
        X_train, X_test, y_train, y_test = train_test_split(X, y, test_size=0.2, shuffle=False)

        params = {
            "objective": "binary",
            "metric": ["auc", "binary_logloss"],
            "boosting_type": "gbdt",
            "learning_rate": 0.03,
            "num_leaves": 31,
            "max_depth": 6,
            "feature_fraction": 0.8,
            "verbose": -1,
            "n_jobs": -1
        }

        train_data = lgb.Dataset(X_train, label=y_train)
        valid_data = lgb.Dataset(X_test, label=y_test, reference=train_data)

        logger.info("Training Scalping LightGBM Production Model...")
        model = lgb.train(
            params,
            train_data,
            num_boost_round=400,
            valid_sets=[valid_data],
        )

        test_preds = model.predict(X_test, num_iteration=model.best_iteration)
        auc_score = roc_auc_score(y_test, test_preds)
        logger.info(f"Model Training Complete! Test AUC: {auc_score:.4f}")

        model.save_model(model_output_path)
        logger.info(f"Saved production model to {model_output_path}")

if __name__ == "__main__":
    # ایجاد دیتای تست با ساختار واقعی جهت ارزیابی پایپ‌لاین
    logger.info("Generating realistic historical ticks with trend & news...")
    timestamps = [datetime.now(timezone.utc) - timedelta(seconds=i) for i in range(20000, 0, -1)]
    
    # شبیه‌سازی قیمت با رانش روند (Drift)
    trend = np.linspace(0, 500, 20000)
    noise = np.cumsum(np.random.randn(20000) * 1.5)
    prices = 65000.0 + trend + noise
    volumes = np.random.exponential(scale=1.5, size=20000) + 0.1
    ofi = np.random.uniform(-1, 1, 20000)

    mock_df = pd.DataFrame({
        "timestamp": timestamps,
        "price": prices,
        "volume": volumes,
        "ofi": ofi,
        "decayed_sentiment": np.random.choice([0.0, 0.5, -0.5, 0.8], size=20000, p=[0.7, 0.1, 0.1, 0.1])
    })

    pipeline = ScalpMultiTimeframePipeline()
    pipeline.train_production_model(mock_df)