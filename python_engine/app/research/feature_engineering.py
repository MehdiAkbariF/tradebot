import numpy as np
import pandas as pd
from loguru import logger

class ScalpFeatureEngineering:
    """
    این کلاس داده‌های خام تیک (Trades/OrderBook) و اخبار را دریافت کرده و 
    برای یادگیری ماشین فیچر (Predictors) و برچسب (Labels) می‌سازد.
    """
    
    def __init__(self, horizon_seconds: int = 60):
        self.horizon_seconds = horizon_seconds

    def generate_features_and_labels(self, ticks_df: pd.DataFrame, news_df: pd.DataFrame) -> pd.DataFrame:
        """
        ticks_df ستون‌های زیر را باید داشته باشد: timestamp, price, volume, ofi
        news_df ستون‌های زیر را باید داشته باشد: timestamp, relevance, surprise
        """
        logger.info("Starting Feature Engineering and Labeling for Scalping...")

        if ticks_df.empty:
            logger.warning("Ticks dataframe is empty.")
            return pd.DataFrame()

        df = ticks_df.sort_values("timestamp").copy()

        # 1. Technical & Microstructure Features
        df["returns_1s"] = df["price"].pct_change(1)
        df["returns_10s"] = df["price"].pct_change(10)
        df["rolling_volatility_30s"] = df["returns_1s"].rolling(30).std()
        df["volume_ma_30s"] = df["volume"].rolling(30).mean()

        # 2. Labeling (Future Return Direction for Scalping)
        # اگر قیمت در افق زمانی مشخص (مثلاً ۶۰ ثانیه بعد) بیشتر از حد مشخصی رشد کرد -> برچسب 1 (BUY)، در غیر این صورت 0
        future_price = df["price"].shift(-self.horizon_seconds)
        price_change_pct = (future_price - df["price"]) / df["price"]

        # آستانه اسکلپینگ (مثلاً 0.05 درصد حرکت مثبت)
        threshold = 0.0005 
        df["target"] = np.where(price_change_pct > threshold, 1, 0)

        # حذف سطوح خالی از انتهای دیتاست که آینده‌شان معلوم نیست
        df = df.dropna().reset_index(drop=True)

        logger.info(f"Feature engineering completed. Total samples: {len(df)}")
        return df

if __name__ == "__main__":
    # تست ساده با داده‌های ساختگی (Mock Data) برای اطمینان از صحت ساختار
    print("Testing Feature Engineering Pipeline...")
    
    dates = pd.date_range(start="2026-01-01", periods=1000, freq="s")
    mock_ticks = pd.DataFrame({
        "timestamp": dates,
        "price": 65000.0 + np.cumsum(np.random.randn(1000) * 2),
        "volume": np.random.uniform(0.1, 2.0, 1000),
        "ofi": np.random.uniform(-1, 1, 1000)
    })

    fe = ScalpFeatureEngineering(horizon_seconds=10)
    dataset = fe.generate_features_and_labels(mock_ticks, pd.DataFrame())
    print(dataset.head())