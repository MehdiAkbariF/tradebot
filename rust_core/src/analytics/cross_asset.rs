pub struct CrossAssetAnalyzer;

impl CrossAssetAnalyzer {
    pub fn compute_lead_lag(leader_returns: &[f64], follower_returns: &[f64], max_lag: usize) -> (isize, f64) {
        if leader_returns.len() != follower_returns.len() || leader_returns.is_empty() {
            return (0, 0.0);
        }

        let mut best_lag = 0;
        let mut max_correlation = -1.0;

        for lag in -(max_lag as isize)..= (max_lag as isize) {
            let corr = Self::shifted_correlation(leader_returns, follower_returns, lag);
            if corr > max_correlation {
                max_correlation = corr;
                best_lag = lag;
            }
        }

        (best_lag, max_correlation)
    }

    fn shifted_correlation(x: &[f64], y: &[f64], lag: isize) -> f64 {
        let len = x.len() as isize;
        let start = std::cmp::max(0, -lag) as usize;
        let end = std::cmp::min(len, len - lag) as usize;

        if start >= end {
            return 0.0;
        }

        let mut sum_x = 0.0;
        let mut sum_y = 0.0;
        let count = (end - start) as f64;

        for i in start..end {
            let x_idx = i as isize;
            let y_idx = i as isize + lag;
            sum_x += x[x_idx as usize];
            sum_y += y[y_idx as usize];
        }

        let mean_x = sum_x / count;
        let mean_y = sum_y / count;

        let mut num = 0.0;
        let mut den_x = 0.0;
        let mut den_y = 0.0;

        for i in start..end {
            let x_idx = i as isize;
            let y_idx = i as isize + lag;
            let xv = x[x_idx as usize] - mean_x;
            let yv = y[y_idx as usize] - mean_y;
            num += xv * yv;
            den_x += xv * xv;
            den_y += yv * yv;
        }

        let den = (den_x * den_y).sqrt();
        if den == 0.0 {
            0.0
        } else {
            num / den
        }
    }
}