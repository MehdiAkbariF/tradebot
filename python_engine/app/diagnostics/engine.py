# مسیر: python_engine/app/diagnostics/engine.py
import json
import os
import numpy as np
import pandas as pd
from datetime import datetime
from loguru import logger

class StrategyDiagnosticEngine:
    def __init__(self, ledger_path: str):
        self.ledger_path = os.path.abspath(ledger_path)

    def load_trades(self) -> pd.DataFrame:
        if not os.path.exists(self.ledger_path):
            return pd.DataFrame()
        try:
            with open(self.ledger_path, "r", encoding="utf-8") as f:
                data = json.load(f)
            if not data:
                return pd.DataFrame()
            return pd.DataFrame(data)
        except Exception as e:
            logger.error(f"Error loading ledger for diagnostics: {e}")
            return pd.DataFrame()

    def generate_full_report(self) -> dict:
        df = self.load_trades()
        if df.empty:
            return {"status": "NO_DATA", "message": "Ledger is empty. Run paper trading to collect samples."}

        # تبدیل انواع داده
        numeric_cols = [
            "gross_pnl", "net_pnl", "total_fee", "duration_seconds",
            "mfe_bps", "mae_bps", "slippage_bps", "model_confidence",
            "alpha_at_signal", "ofi_at_signal", "range_at_signal_bps"
        ]
        for col in numeric_cols:
            if col in df.columns:
                df[col] = pd.to_numeric(df[col], errors="coerce").fillna(0.0)

        total_trades = len(df)
        wins = df[df["net_pnl"] > 0]
        losses = df[df["net_pnl"] < 0]
        scratches = df[df["net_pnl"] == 0]

        gross_pnl = df["gross_pnl"].sum()
        total_fees = df["total_fee"].sum()
        net_pnl = df["net_pnl"].sum()

        win_rate = (len(wins) / total_trades) * 100.0 if total_trades > 0 else 0.0
        profit_factor = (wins["net_pnl"].sum() / abs(losses["net_pnl"].sum())) if len(losses) > 0 and losses["net_pnl"].sum() != 0 else float("inf")

        # امید ریاضی (Expectancy) قبل و بعد از هزینه
        avg_win_net = wins["net_pnl"].mean() if len(wins) > 0 else 0.0
        avg_loss_net = abs(losses["net_pnl"].mean()) if len(losses) > 0 else 0.0
        net_expectancy = ((len(wins) / total_trades) * avg_win_net) - ((len(losses) / total_trades) * avg_loss_net)

        avg_win_gross = wins["gross_pnl"].mean() if len(wins) > 0 else 0.0
        avg_loss_gross = abs(losses["gross_pnl"].mean()) if len(losses) > 0 else 0.0
        gross_expectancy = ((len(wins) / total_trades) * avg_win_gross) - ((len(losses) / total_trades) * avg_loss_gross)

        # تحلیل MFE / MAE
        avg_mfe = df["mfe_bps"].mean() if "mfe_bps" in df.columns else 0.0
        avg_mae = df["mae_bps"].mean() if "mae_bps" in df.columns else 0.0

        # ۱. تفکیک عملکرد بر اساس دلایل خروج (Exit Attribution)
        exit_breakdown = {}
        if "exit_reason" in df.columns:
            for reason, group in df.groupby("exit_reason"):
                exit_breakdown[reason] = {
                    "count": len(group),
                    "win_rate": round((len(group[group["net_pnl"] > 0]) / len(group)) * 100.0, 2),
                    "net_pnl": round(float(group["net_pnl"].sum()), 2),
                    "avg_mfe_bps": round(float(group["mfe_bps"].mean()), 2) if "mfe_bps" in group.columns else 0.0,
                    "avg_mae_bps": round(float(group["mae_bps"].mean()), 2) if "mae_bps" in group.columns else 0.0,
                }

        # ۲. تفکیک بر اساس نماد معاملاتی (Symbol Attribution)
        symbol_breakdown = {}
        if "symbol" in df.columns:
            for sym, group in df.groupby("symbol"):
                symbol_breakdown[sym] = {
                    "trades": len(group),
                    "win_rate": round((len(group[group["net_pnl"] > 0]) / len(group)) * 100.0, 2),
                    "net_pnl": round(float(group["net_pnl"].sum()), 2),
                    "expectancy": round(float(group["net_pnl"].mean()), 4),
                }

        # ۳. تفکیک جهت معامله (Directional Attribution)
        direction_breakdown = {}
        if "action" in df.columns:
            for act, group in df.groupby("action"):
                direction_breakdown[act] = {
                    "trades": len(group),
                    "win_rate": round((len(group[group["net_pnl"] > 0]) / len(group)) * 100.0, 2),
                    "net_pnl": round(float(group["net_pnl"].sum()), 2),
                }

        # ۴. تحلیل کالیبراسیون اطمینان (Confidence Calibration)
        confidence_calibration = {}
        if "model_confidence" in df.columns:
            bins = [0.5, 0.6, 0.7, 0.8, 0.9, 0.95, 1.01]
            labels = ["50-60%", "60-70%", "70-80%", "80-90%", "90-95%", "95-100%"]
            df["conf_bin"] = pd.cut(df["model_confidence"], bins=bins, labels=labels, right=False)
            for b_name, group in df.groupby("conf_bin", observed=False):
                if len(group) > 0:
                    w_rate = (len(group[group["net_pnl"] > 0]) / len(group)) * 100.0
                    confidence_calibration[str(b_name)] = {
                        "trades": len(group),
                        "observed_win_rate": round(w_rate, 2),
                        "expectancy": round(float(group["net_pnl"].mean()), 4)
                    }

        # ۵. تحلیل آماری تایم‌اوت‌ها (TIME_EXPIRATION Deep Dive)
        timeout_trades = df[df["exit_reason"].str.contains("TIME_EXPIRATION", na=False)] if "exit_reason" in df.columns else pd.DataFrame()
        timeout_stats = {}
        if not timeout_trades.empty:
            timeout_stats = {
                "count": len(timeout_trades),
                "ratio_of_total": round((len(timeout_trades) / total_trades) * 100.0, 2),
                "total_loss": round(float(timeout_trades["net_pnl"].sum()), 2),
                "avg_mfe_bps": round(float(timeout_trades["mfe_bps"].mean()), 2) if "mfe_bps" in timeout_trades.columns else 0.0,
                "avg_mae_bps": round(float(timeout_trades["mae_bps"].mean()), 2) if "mae_bps" in timeout_trades.columns else 0.0,
            }

        # ۶. عیب‌یابی علت حاکم (Dominant Failure Mode Classification)
        failure_mode = "NONE (Profitable Edge Proven)"
        if total_trades < 100:
            failure_mode = "INSUFFICIENT_DATA (Need >= 300 samples for statistical validity)"
        elif gross_pnl > 0 and net_pnl <= 0:
            failure_mode = "EXECUTION_ECONOMICS (Edge exists before fees, but fee/slippage destroys it)"
        elif timeout_stats.get("ratio_of_total", 0) > 40 and timeout_stats.get("avg_mfe_bps", 0) > 10.0:
            failure_mode = "EXIT_INEFFICIENCY (Signals reach substantial profit but exit logic misses them)"
        elif avg_mae < -12.0 and avg_mfe < 4.0:
            failure_mode = "PREDICTIVE_FAILURE (Signal direction is statistically invalid)"

        return {
            "summary": {
                "total_trades": total_trades,
                "win_rate": round(win_rate, 2),
                "profit_factor": round(profit_factor, 2) if profit_factor != float("inf") else 999.0,
                "gross_pnl": round(float(gross_pnl), 2),
                "total_fees": round(float(total_fees), 2),
                "net_pnl": round(float(net_pnl), 2),
                "gross_expectancy": round(float(gross_expectancy), 4),
                "net_expectancy": round(float(net_expectancy), 4),
                "avg_mfe_bps": round(float(avg_mfe), 2),
                "avg_mae_bps": round(float(avg_mae), 2),
                "dominant_failure_mode": failure_mode
            },
            "attribution": {
                "exits": exit_breakdown,
                "symbols": symbol_breakdown,
                "directions": direction_breakdown,
                "confidence_calibration": confidence_calibration,
                "timeout_analysis": timeout_stats
            }
        }