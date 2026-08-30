# مسیر: python_engine/app/diagnostics/engine.py
import json
import os
import numpy as np
import pandas as pd
from loguru import logger

class StrategyDiagnosticEngine:
    def __init__(self, ledger_path: str):
        self.ledger_path = os.path.abspath(ledger_path)

    def load_canonical_ledger(self) -> pd.DataFrame:
        if not os.path.exists(self.ledger_path):
            return pd.DataFrame()
        try:
            with open(self.ledger_path, "r", encoding="utf-8") as f:
                data = json.load(f)
            return pd.DataFrame(data) if data else pd.DataFrame()
        except Exception as e:
            logger.error(f"Error loading canonical ledger: {e}")
            return pd.DataFrame()

    def generate_full_report(self) -> dict:
        df = self.load_canonical_ledger()
        if df.empty:
            return {"status": "AWAITING_DATA", "message": "Accumulating audited executions from live market."}

        numeric_fields = [
            "gross_pnl", "net_pnl", "total_fee", "pnl_bps", 
            "mfe_bps", "mae_bps", "expected_net_ev_bps", "duration_seconds"
        ]
        for f in numeric_fields:
            if f in df.columns:
                df[f] = pd.to_numeric(df[f], errors="coerce").fillna(0.0)

        total_trades = len(df)
        wins = df[df["net_pnl"] > 0.0]
        losses = df[df["net_pnl"] < 0.0]
        timeouts = df[df["exit_reason"].str.contains("TIME_EXPIRATION", na=False)]
        tp_hits = df[df["exit_reason"].str.contains("TP", na=False)]
        sl_hits = df[df["exit_reason"].str.contains("STOP_LOSS", na=False)]

        win_rate = (len(wins) / total_trades) * 100.0 if total_trades > 0 else 0.0
        gross_alpha_usd = float(df["gross_pnl"].sum())
        total_friction_usd = float(df["total_fee"].sum())
        net_realized_usd = float(df["net_pnl"].sum())

        sum_wins = float(wins["net_pnl"].sum())
        sum_losses = abs(float(losses["net_pnl"].sum()))
        profit_factor = (sum_wins / sum_losses) if sum_losses > 0 else (sum_wins if sum_wins > 0 else 0.0)

        realized_pnl_bps = df["pnl_bps"].values if "pnl_bps" in df.columns else np.zeros(total_trades)
        mean_realized_bps = float(np.mean(realized_pnl_bps)) if len(realized_pnl_bps) > 0 else 0.0
        std_realized_bps = float(np.std(realized_pnl_bps)) if len(realized_pnl_bps) > 1 else 1e-6

        predicted_ev_bps = float(df["expected_net_ev_bps"].mean()) if "expected_net_ev_bps" in df.columns else 0.0
        calibration_error_bps = round(abs(predicted_ev_bps - mean_realized_bps), 2)

        # ماتریس کالیبراسیون باکت‌بندی‌شده
        bins = [1.0, 1.5, 2.0, 2.5, 3.0, 3.5, 100.0]
        labels = ["1.0-1.5", "1.5-2.0", "2.0-2.5", "2.5-3.0", "3.0-3.5", "3.5+"]
        
        binned_calibration = {}
        if "expected_net_ev_bps" in df.columns:
            df["ev_bucket"] = pd.cut(df["expected_net_ev_bps"], bins=bins, labels=labels, right=False)
            for b_label, group in df.groupby("ev_bucket", observed=False):
                if len(group) > 0:
                    group_wins = group[group["net_pnl"] > 0.0]
                    binned_calibration[str(b_label)] = {
                        "trades": len(group),
                        "predicted_ev": round(float(group["expected_net_ev_bps"].mean()), 2),
                        "realized_ev": round(float(group["pnl_bps"].mean()), 2),
                        "win_rate": round((len(group_wins) / len(group)) * 100.0, 1),
                        "avg_mfe_directional": round(float(group["mfe_bps"].mean()), 1),
                        "avg_mae_directional": round(float(group["mae_bps"].mean()), 1)
                    }

        if total_trades < 100:
            verdict = "INSUFFICIENT_SAMPLE (Accumulating >= 100 executions)"
        elif mean_realized_bps >= 1.2 and profit_factor >= 1.30 and calibration_error_bps <= 1.0:
            verdict = "PROVEN_POSITIVE_EDGE (Empirically Validated)"
        elif mean_realized_bps > 0 and profit_factor >= 1.05:
            verdict = "WEAK_PROMISING_EDGE (Execution Friction high)"
        elif gross_alpha_usd > 0 and net_realized_usd <= 0:
            verdict = "FRICTION_DOMINATED (Gross Alpha wiped out by Execution Fees)"
        else:
            verdict = "NO_EDGE_REJECTED (Negative Expectancy)"

        return {
            "audit_header": "══════════════════════ CANONICAL QUANT AUDIT v9.0 ══════════════════════",
            "execution_summary": {
                "executed_trades": total_trades,
                "wins": len(wins),
                "losses": len(losses),
                "tp_rate_pct": round((len(tp_hits) / total_trades) * 100.0, 2) if total_trades > 0 else 0.0,
                "sl_rate_pct": round((len(sl_hits) / total_trades) * 100.0, 2) if total_trades > 0 else 0.0,
                "timeout_rate_pct": round((len(timeouts) / total_trades) * 100.0, 2) if total_trades > 0 else 0.0,
                "win_rate_pct": round(win_rate, 2),
                "profit_factor": round(profit_factor, 2),
                "gross_alpha_usd": round(gross_alpha_usd, 4),
                "total_friction_usd": round(total_friction_usd, 4),
                "net_realized_usd": round(net_realized_usd, 4),
            },
            "expectancy_and_calibration": {
                "predicted_mean_ev_bps": round(predicted_ev_bps, 2),
                "realized_mean_ev_bps": round(mean_realized_bps, 2),
                "ev_calibration_error_bps": calibration_error_bps,
                "binned_calibration_matrix": binned_calibration
            },
            "path_dependency_attribution": {
                "avg_mfe_directional_bps": round(float(df["mfe_bps"].mean()), 2) if "mfe_bps" in df.columns else 0.0,
                "avg_mae_directional_bps": round(float(df["mae_bps"].mean()), 2) if "mae_bps" in df.columns else 0.0,
            },
            "verdict": {
                "edge_status": verdict
            }
        }