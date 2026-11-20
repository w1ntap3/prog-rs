//! Currencies and conversion rates shared by finance data and app settings.
//!
//! Everything the app *shows* is CZK. Transactions remember what was typed
//! (amount + currency) and a canonical CZK value fixed at entry time, so
//! editing rates later never rewrites history.

use serde::{Deserialize, Serialize};

/// Currency the user typed an amount in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Currency {
    Czk,
    Eur,
    Azn,
}

impl Currency {
    pub const ALL: [Currency; 3] = [Currency::Czk, Currency::Eur, Currency::Azn];

    pub fn label(self) -> &'static str {
        match self {
            Currency::Czk => "CZK",
            Currency::Eur => "EUR",
            Currency::Azn => "AZN",
        }
    }
}

/// User-supplied conversion rates: how many CZK one unit is worth.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rates {
    pub eur: f64,
    pub azn: f64,
}

impl Default for Rates {
    fn default() -> Self {
        Self { eur: 0.0, azn: 0.0 }
    }
}

impl Rates {
    pub fn czk_per(self, currency: Currency) -> f64 {
        match currency {
            Currency::Czk => 1.0,
            Currency::Eur => self.eur,
            Currency::Azn => self.azn,
        }
    }

    pub fn set(&mut self, currency: Currency, rate: f64) {
        match currency {
            Currency::Czk => {}
            Currency::Eur => self.eur = rate,
            Currency::Azn => self.azn = rate,
        }
    }

    pub fn is_valid(self) -> bool {
        self.eur.is_finite() && self.azn.is_finite() && self.eur >= 0.0 && self.azn >= 0.0
    }
}

/// Accepts `1 200`, `1 200.50` or `1200,50`; returns the number.
pub fn parse_decimal(raw: &str) -> Option<f64> {
    let cleaned: String = raw
        .chars()
        .filter(|c| !c.is_whitespace())
        .map(|c| if c == ',' { '.' } else { c })
        .collect();
    if cleaned.is_empty() {
        None
    } else {
        cleaned.parse().ok()
    }
}

/// `25.30` → `25.3`, `25.00` → `25`.
pub fn trim_decimal(value: f64) -> String {
    let text = format!("{value:.2}");
    text.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// Rates are shown with up to four decimals, hidden when unset (0).
pub fn rate_string(rate: f64) -> String {
    if rate <= 0.0 {
        String::new()
    } else {
        let text = format!("{rate:.4}");
        text.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}
