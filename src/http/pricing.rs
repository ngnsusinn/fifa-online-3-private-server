use std::collections::HashMap;
use crate::http::constants::GRADE_PRICE;

/// Rounds `x` to 3 significant digits matching Python `http_config.py` sig3 algorithm:
/// `round(x, 2 - len(str(x))) if x >= 1000 else x`
pub fn sig3(x: f64) -> i64 {
    let xi = x as i64;
    if xi < 1000 {
        return xi;
    }

    let s = xi.to_string();
    let len = s.len() as i32;
    let k = (len - 2) as u32;
    let factor = 10_f64.powi(k as i32);
    let rounded = (x / factor).round_ties_even();
    (rounded * factor) as i64
}

/// Fallback without gamedata/fa_prices.json:
/// Least-squares fit of fifaaddict Indonesia-server prices on client rating:
/// `sig3(max(1000, exp(2.53 + 0.1113 * ovr)))`
/// 65 -> ~17k, 85 -> ~160k
pub fn price_formula(ovr: u32) -> i64 {
    let raw = 1000.0_f64.max((2.53 + 0.1113 * (ovr as f64)).exp());
    sig3(raw)
}

/// EP price at +1 of a card rated `ovr`.
/// Uses `fa_table` (rating-to-price mapping) if it contains at least 6 entries,
/// otherwise falls back to `price_formula(ovr)`.
pub fn stat_price(ovr: u32, fa_table: Option<&HashMap<u32, f64>>) -> i64 {
    if let Some(t) = fa_table {
        if t.len() >= 6 {
            let mut keys: Vec<u32> = t.keys().copied().collect();
            keys.sort();
            let lo = *keys.first().unwrap();
            let hi = *keys.last().unwrap();

            if ovr > hi {
                let hi_val = *t.get(&hi).unwrap_or(&0.0);
                let hi5_key = hi.saturating_sub(5);
                let hi5_val = *t.get(&hi5_key).unwrap_or(&hi_val);
                let ratio = if hi5_val > 0.0 { hi_val / hi5_val } else { 1.0 };
                let exp = (ovr - hi) as f64 / 5.0;
                return (hi_val * ratio.powf(exp)).round() as i64;
            }

            let target = ovr.max(lo);
            if let Some(&price) = t.get(&target) {
                return price.round() as i64;
            }
        }
    }

    price_formula(ovr)
}

/// EP value from card's rating and grade:
/// `sig3(stat_price(ovr) * GRADE_PRICE[grade])`.
/// Grade +0 yields 0 EP.
pub fn card_price(card_ovr: u32, grade: u32, fa_table: Option<&HashMap<u32, f64>>) -> i64 {
    let g = grade.min(10) as usize;
    let mult = GRADE_PRICE[g];
    if mult <= 0.0 {
        return 0;
    }
    let base = stat_price(card_ovr, fa_table);
    sig3(base as f64 * mult)
}

/// Convenience function to calculate card price directly from SPID and grade.
/// Uses `fifadb.sqlite` (or fallback 75) for the card's rating.
pub fn card_price_spid(spid: u64, grade: u32) -> i64 {
    let ovr = crate::state::card_ovr(spid);
    card_price(ovr, grade, None)
}

/// Computes median of a slice of values
fn median(v: &mut [f64]) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mid = v.len() / 2;
    if v.len() % 2 == 0 {
        (v[mid - 1] + v[mid]) / 2.0
    } else {
        v[mid]
    }
}

/// Builds rating price curve from fifaaddict prices and card ratings
pub fn build_rating_prices(
    fa_prices: &HashMap<u32, u64>,
    card_ovrs: &HashMap<u32, u32>,
) -> HashMap<u32, f64> {
    let mut by_ovr: HashMap<u32, Vec<f64>> = HashMap::new();
    for (&spid, &price) in fa_prices {
        if price > 0 {
            if let Some(&ovr) = card_ovrs.get(&spid) {
                if ovr > 0 {
                    by_ovr.entry(ovr).or_default().push(price as f64);
                }
            }
        }
    }

    let mut medians: HashMap<u32, f64> = HashMap::new();
    for (ovr, mut prices) in by_ovr {
        if prices.len() >= 5 {
            medians.insert(ovr, median(&mut prices));
        }
    }

    if medians.is_empty() {
        return HashMap::new();
    }

    let mut keys: Vec<u32> = medians.keys().copied().collect();
    keys.sort();
    let min_ovr = *keys.first().unwrap();
    let max_ovr = *keys.last().unwrap();

    let mut table = HashMap::new();
    let mut top: f64 = 0.0;
    for o in min_ovr..=max_ovr {
        let m = medians.get(&o).copied().unwrap_or(0.0);
        top = (top * 1.05f64).max(m);
        table.insert(o, top);
    }

    table
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sig3() {
        assert_eq!(sig3(500.0), 500);
        assert_eq!(sig3(999.0), 999);
        assert_eq!(sig3(1000.0), 1000);
        assert_eq!(sig3(1234.0), 1200);
        assert_eq!(sig3(15678.0), 16000);
        assert_eq!(sig3(123456.0), 120000);
        assert_eq!(sig3(999999.0), 1000000);
        assert_eq!(sig3(125000.0), 120000);
        assert_eq!(sig3(135000.0), 140000);
    }

    #[test]
    fn test_price_formula() {
        let p65 = price_formula(65);
        assert_eq!(p65, 17000);
        let p85 = price_formula(85);
        assert_eq!(p85, 160000);
    }

    #[test]
    fn test_card_price() {
        assert_eq!(card_price(65, 0, None), 0);
        assert_eq!(card_price(65, 1, None), 17000);
        assert!(card_price(65, 2, None) > 17000);
        assert!(card_price(65, 5, None) > card_price(65, 2, None));
        assert_eq!(card_price(83, 1, None), 130000);
        assert_eq!(card_price(83, 10, None), 820000000);
    }

    #[test]
    fn test_card_price_spid() {
        assert_eq!(card_price_spid(88184941, 0), 0);
        let p1 = card_price_spid(88184941, 1);
        assert!(p1 > 0);
        let p2 = card_price_spid(88184941, 2);
        assert!(p2 > p1);
        let p10 = card_price_spid(88184941, 10);
        assert!(p10 > p2);
    }
}
