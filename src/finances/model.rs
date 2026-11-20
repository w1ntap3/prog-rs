//! Finance records: the private data file behind the `finances` section.
//!
//! This is personal content, so it lives in the data directory (never in the
//! repo) and is only moved by an explicit user export. The model is a list of
//! payout windows (each with per-category balances) plus a flat, human-editable
//! transaction log. Everything else (daily thresholds, baselines, gaps) is
//! derived in `stats`.
//!
//! Foreign-currency amounts are converted to CZK at entry time and stored in
//! both forms, so later rate changes never rewrite history.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use chrono::{Datelike, Local, NaiveDate, NaiveTime};
use serde::{Deserialize, Serialize};

use crate::currency::Currency;
use crate::data;

pub const VERSION: u32 = 6;
const FILE: &str = "finances.toml";

/// Spending categories. Fixed for now; can move to config if they grow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Category {
    Nutrition,
    Allowance,
}

impl Category {
    pub const ALL: [Category; 2] = [Category::Nutrition, Category::Allowance];

    pub fn label(self) -> &'static str {
        match self {
            Category::Nutrition => "nutrition",
            Category::Allowance => "allowance",
        }
    }
}

/// One category's allocated balance for a payout window.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CategoryBudget {
    pub category: Category,
    /// Whole CZK available for this category in this window.
    pub amount_czk: i64,
}

/// A payout window: last payday → next payday (exclusive).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Period {
    pub start: NaiveDate,
    pub end: NaiveDate,
    /// One-pot window: categories are ignored and the whole balance is shared.
    #[serde(default)]
    pub combined: bool,
    /// Balance for a combined window. `None` falls back to the sum of the
    /// per-category balances, so marking an existing window as combined never
    /// loses its money.
    #[serde(default)]
    pub combined_budget: Option<i64>,
    #[serde(default)]
    pub budgets: Vec<CategoryBudget>,
}

impl Period {
    pub fn days(&self) -> i64 {
        (self.end - self.start).num_days().max(1)
    }

    /// The raw per-category balance (what the split editor shows).
    pub fn budget(&self, category: Category) -> i64 {
        self.budgets
            .iter()
            .find(|budget| budget.category == category)
            .map(|budget| budget.amount_czk)
            .unwrap_or(0)
    }

    pub fn total_budget(&self) -> i64 {
        self.budgets.iter().map(|budget| budget.amount_czk).sum()
    }

    pub fn combined_balance(&self) -> i64 {
        self.combined_budget.unwrap_or_else(|| self.total_budget())
    }

    /// The balance that applies to the window's actual math.
    pub fn balance(&self, category: Category) -> i64 {
        if self.combined {
            self.combined_balance()
        } else {
            self.budget(category)
        }
    }

    pub fn set_combined_balance(&mut self, amount_czk: i64) {
        self.combined_budget = Some(amount_czk);
    }

    pub fn set_budget(&mut self, category: Category, amount_czk: i64) {
        if let Some(budget) = self
            .budgets
            .iter_mut()
            .find(|budget| budget.category == category)
        {
            budget.amount_czk = amount_czk;
        } else {
            self.budgets.push(CategoryBudget {
                category,
                amount_czk,
            });
        }
    }
}

/// Whether money left or arrived.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Expense,
    Income,
}

impl Kind {
    pub fn label(self) -> &'static str {
        match self {
            Kind::Expense => "expense",
            Kind::Income => "income",
        }
    }

    pub fn verb(self) -> &'static str {
        match self {
            Kind::Expense => "spent",
            Kind::Income => "gained",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Transaction {
    pub date: NaiveDate,
    /// Local clock time; kept for ordering and for the ledger display.
    pub time: NaiveTime,
    /// Canonical value in whole CZK, fixed when the transaction was entered.
    pub amount_czk: i64,
    /// What the user typed, in `currency` (equal to `amount_czk` for CZK).
    pub original_amount: f64,
    pub currency: Currency,
    pub category: Category,
    pub kind: Kind,
    #[serde(default)]
    pub note: String,
}

/// A manual reconciliation: force the balance at a timestamp to the real
/// amount. Stored as a signed CZK delta so every later point shifts with it,
/// without pretending it was a purchase or income.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Adjustment {
    pub date: NaiveDate,
    pub time: NaiveTime,
    /// Positive raises the balance, negative lowers it.
    pub delta_czk: i64,
    pub category: Category,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FinanceData {
    pub version: u32,
    #[serde(default)]
    pub periods: Vec<Period>,
    #[serde(default)]
    pub transactions: Vec<Transaction>,
    #[serde(default)]
    pub adjustments: Vec<Adjustment>,
}

impl Default for FinanceData {
    fn default() -> Self {
        let today = Local::now().date_naive();
        let mut period = Period {
            start: today,
            end: add_months(today, 1),
            combined: false,
            combined_budget: None,
            budgets: Vec::new(),
        };
        for category in Category::ALL {
            period.set_budget(category, 0);
        }
        Self {
            version: VERSION,
            periods: vec![period],
            transactions: Vec::new(),
            adjustments: Vec::new(),
        }
    }
}

impl FinanceData {
    pub fn file_path() -> PathBuf {
        data::dir().join(FILE)
    }

    pub fn load_from(path: &Path) -> Result<Self> {
        let text = fs::read_to_string(path)
            .with_context(|| format!("reading {}", path.display()))?;
        let probe: VersionProbe =
            toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
        let data = match probe.version {
            VERSION => toml::from_str::<FinanceData>(&text)
                .with_context(|| format!("parsing {}", path.display()))?,
            5 => {
                let old: V5 = toml::from_str(&text)
                    .with_context(|| format!("migrating {}", path.display()))?;
                migrate_v5(old)
            }
            4 => {
                let old: V4 = toml::from_str(&text)
                    .with_context(|| format!("migrating {}", path.display()))?;
                migrate_v4(old)
            }
            3 => {
                let old: V3 = toml::from_str(&text)
                    .with_context(|| format!("migrating {}", path.display()))?;
                migrate_v3(old)
            }
            2 => {
                let old: V2 = toml::from_str(&text)
                    .with_context(|| format!("migrating {}", path.display()))?;
                migrate_v2(old)
            }
            1 => {
                let old: V1 = toml::from_str(&text)
                    .with_context(|| format!("migrating {}", path.display()))?;
                migrate_v1(old)
            }
            other => bail!("unsupported finance data version {other} (expected {VERSION})"),
        };
        data.validate()?;
        Ok(data)
    }

    /// Load the live file; create it with defaults on first use.
    pub fn load_or_create() -> Result<Self> {
        let path = Self::file_path();
        if path.exists() {
            return Self::load_from(&path);
        }
        let data = Self::default();
        data.save()?;
        Ok(data)
    }

    pub fn save_to(&self, path: &Path) -> Result<()> {
        self.validate()?;
        if let Some(dir) = path.parent() {
            crate::paths::ensure_dir(dir)?;
        }
        fs::write(path, toml::to_string_pretty(self)?)
            .with_context(|| format!("writing {}", path.display()))
    }

    pub fn save(&self) -> Result<()> {
        self.save_to(&Self::file_path())
    }

    /// Index of the window containing `today`, or the last one if today is
    /// past every stored window.
    pub fn current_period_index(&self, today: NaiveDate) -> usize {
        self.periods
            .iter()
            .position(|period| today >= period.start && today < period.end)
            .unwrap_or_else(|| self.periods.len().saturating_sub(1))
    }

    fn validate(&self) -> Result<()> {
        if self.version != VERSION {
            bail!("unsupported finance data version {} (expected {VERSION})", self.version);
        }
        if self.periods.is_empty() {
            bail!("finance data has no payout windows");
        }
        for (i, period) in self.periods.iter().enumerate() {
            if period.start >= period.end {
                bail!("window {} ends before it starts", i + 1);
            }
            for budget in &period.budgets {
                if budget.amount_czk < 0 {
                    bail!("window {} has a negative category balance", i + 1);
                }
            }
            if period.combined_budget.is_some_and(|balance| balance < 0) {
                bail!("window {} has a negative combined balance", i + 1);
            }
        }
        for (i, tx) in self.transactions.iter().enumerate() {
            if tx.amount_czk <= 0 {
                bail!("transaction {} has a non-positive CZK amount", i + 1);
            }
            if !tx.original_amount.is_finite() || tx.original_amount <= 0.0 {
                bail!("transaction {} has a non-positive original amount", i + 1);
            }
        }
        Ok(())
    }
}

/// The day the salary lands in a given month, clamped to short months.
fn payday(year: i32, month: u32, requested_day: u32) -> NaiveDate {
    for day in (1..=requested_day.clamp(1, 31)).rev() {
        if let Some(date) = NaiveDate::from_ymd_opt(year, month, day) {
            return date;
        }
    }
    NaiveDate::from_ymd_opt(year, month, 1).expect("first of month is always valid")
}

fn shift_month(year: i32, month: u32, delta: i32) -> (i32, u32) {
    let index = year * 12 + month as i32 - 1 + delta;
    (index.div_euclid(12), (index.rem_euclid(12) + 1) as u32)
}

/// Add calendar months, clamping the day for short months.
pub fn add_months(date: NaiveDate, delta: i32) -> NaiveDate {
    let (year, month) = shift_month(date.year(), date.month(), delta);
    payday(year, month, date.day())
}

pub fn first_of_month(date: NaiveDate) -> NaiveDate {
    NaiveDate::from_ymd_opt(date.year(), date.month(), 1).expect("first of month is always valid")
}

pub fn days_in_month(date: NaiveDate) -> u32 {
    let (year, month) = shift_month(date.year(), date.month(), 1);
    NaiveDate::from_ymd_opt(year, month, 1)
        .expect("first of next month is always valid")
        .pred_opt()
        .expect("first of next month has a predecessor")
        .day()
}

#[derive(Deserialize)]
struct VersionProbe {
    version: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
enum LegacyKind {
    Income,
    Expense,
}

/// Build a single-window v5 dataset from pre-v5 data. Pre-v5 had no
/// categories, so all expenses land in `allowance`; income transactions are
/// dropped because windows now carry explicit balances.
fn legacy_data(
    start: NaiveDate,
    end: NaiveDate,
    transactions: Vec<(NaiveDate, NaiveTime, i64, f64, Currency, LegacyKind, String)>,
) -> FinanceData {
    let mut period = Period {
        start,
        end,
        combined: false,
        combined_budget: None,
        budgets: Vec::new(),
    };
    for category in Category::ALL {
        period.set_budget(category, 0);
    }
    let transactions = transactions
        .into_iter()
        .map(
            |(date, time, amount_czk, original_amount, currency, kind, note)| Transaction {
                date,
                time,
                amount_czk,
                original_amount,
                currency,
                category: Category::Allowance,
                kind: match kind {
                    LegacyKind::Expense => Kind::Expense,
                    LegacyKind::Income => Kind::Income,
                },
                note,
            },
        )
        .collect();
    FinanceData {
        version: VERSION,
        periods: vec![period],
        transactions,
        adjustments: Vec::new(),
    }
}

/// v5 had windows and categories, but every transaction was an expense.
#[derive(Deserialize)]
struct V5 {
    #[allow(dead_code, reason = "version marker")]
    version: u32,
    #[serde(default)]
    periods: Vec<Period>,
    #[serde(default)]
    transactions: Vec<V5Transaction>,
}

#[derive(Deserialize)]
struct V5Transaction {
    date: NaiveDate,
    time: NaiveTime,
    amount_czk: i64,
    original_amount: f64,
    currency: Currency,
    category: Category,
    #[serde(default)]
    note: String,
}

fn migrate_v5(old: V5) -> FinanceData {
    let transactions = old
        .transactions
        .into_iter()
        .map(|tx| Transaction {
            date: tx.date,
            time: tx.time,
            amount_czk: tx.amount_czk,
            original_amount: tx.original_amount,
            currency: tx.currency,
            category: tx.category,
            kind: Kind::Expense,
            note: tx.note,
        })
        .collect();
    FinanceData {
        version: VERSION,
        periods: old.periods,
        transactions,
        adjustments: Vec::new(),
    }
}

/// v4 was the CZK-only, explicit-payday shape introduced with currencies.
#[derive(Deserialize)]
struct V4 {
    #[allow(dead_code, reason = "version marker")]
    version: u32,
    last_payday: NaiveDate,
    next_payday: NaiveDate,
    #[serde(default)]
    transactions: Vec<V4Transaction>,
}

#[derive(Deserialize)]
struct V4Transaction {
    date: NaiveDate,
    time: NaiveTime,
    amount_czk: i64,
    original_amount: f64,
    currency: Currency,
    kind: LegacyKind,
    #[serde(default)]
    note: String,
}

fn migrate_v4(old: V4) -> FinanceData {
    legacy_data(
        old.last_payday,
        old.next_payday,
        old.transactions
            .into_iter()
            .map(|tx| {
                (
                    tx.date,
                    tx.time,
                    tx.amount_czk,
                    tx.original_amount,
                    tx.currency,
                    tx.kind,
                    tx.note,
                )
            })
            .collect(),
    )
}

/// v3 added currencies; v2 was CZK-only. Both used explicit paydays.
#[derive(Deserialize)]
struct V3 {
    #[allow(dead_code, reason = "version marker")]
    version: u32,
    last_payday: NaiveDate,
    next_payday: NaiveDate,
    #[serde(default)]
    transactions: Vec<V3Transaction>,
}

#[derive(Deserialize)]
struct V3Transaction {
    date: NaiveDate,
    time: NaiveTime,
    amount_czk: i64,
    original_amount: f64,
    currency: Currency,
    kind: LegacyKind,
    #[serde(default)]
    note: String,
}

fn migrate_v3(old: V3) -> FinanceData {
    legacy_data(
        old.last_payday,
        old.next_payday,
        old.transactions
            .into_iter()
            .map(|tx| {
                (
                    tx.date,
                    tx.time,
                    tx.amount_czk,
                    tx.original_amount,
                    tx.currency,
                    tx.kind,
                    tx.note,
                )
            })
            .collect(),
    )
}

#[derive(Deserialize)]
struct V2 {
    #[allow(dead_code, reason = "version marker")]
    version: u32,
    last_payday: NaiveDate,
    next_payday: NaiveDate,
    #[serde(default)]
    transactions: Vec<V2Transaction>,
}

#[derive(Deserialize)]
struct V2Transaction {
    date: NaiveDate,
    time: NaiveTime,
    amount: i64,
    kind: LegacyKind,
    #[serde(default)]
    note: String,
}

fn migrate_v2(old: V2) -> FinanceData {
    legacy_data(
        old.last_payday,
        old.next_payday,
        old.transactions
            .into_iter()
            .map(|tx| {
                (
                    tx.date,
                    tx.time,
                    tx.amount,
                    tx.amount as f64,
                    Currency::Czk,
                    tx.kind,
                    tx.note,
                )
            })
            .collect(),
    )
}

/// v1 kept a day-of-month payday and a salary fallback.
#[derive(Deserialize)]
struct V1 {
    #[allow(dead_code, reason = "version marker")]
    version: u32,
    payday_day: u32,
    #[allow(dead_code, reason = "dropped in v2")]
    #[serde(default)]
    salary: i64,
    #[allow(dead_code, reason = "dropped in v2")]
    #[serde(default)]
    initial_savings: i64,
    #[serde(default)]
    transactions: Vec<V1Transaction>,
}

#[derive(Deserialize)]
struct V1Transaction {
    date: NaiveDate,
    amount: i64,
    kind: String,
    #[serde(default)]
    note: String,
}

fn migrate_v1(old: V1) -> FinanceData {
    let today = Local::now().date_naive();
    let this_month = payday(today.year(), today.month(), old.payday_day);
    let (start, end) = if today >= this_month {
        let (year, month) = shift_month(today.year(), today.month(), 1);
        (this_month, payday(year, month, old.payday_day))
    } else {
        let (year, month) = shift_month(today.year(), today.month(), -1);
        (payday(year, month, old.payday_day), this_month)
    };

    let noon = NaiveTime::from_hms_opt(12, 0, 0).expect("noon is valid");
    legacy_data(
        start,
        end,
        old.transactions
            .into_iter()
            .filter(|tx| tx.kind != "saving")
            .map(|tx| {
                (
                    tx.date,
                    noon,
                    tx.amount,
                    tx.amount as f64,
                    Currency::Czk,
                    if tx.kind == "income" {
                        LegacyKind::Income
                    } else {
                        LegacyKind::Expense
                    },
                    tx.note,
                )
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("progrs-fin-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    fn period(start: NaiveDate, end: NaiveDate, nutrition: i64, allowance: i64) -> Period {
        let mut period = Period {
            start,
            end,
            combined: false,
            combined_budget: None,
            budgets: Vec::new(),
        };
        period.set_budget(Category::Nutrition, nutrition);
        period.set_budget(Category::Allowance, allowance);
        period
    }

    #[test]
    fn roundtrip_through_file() {
        let dir = temp("roundtrip");
        let path = dir.join(FILE);
        let data = FinanceData {
            version: VERSION,
            periods: vec![
                period(date(2026, 10, 15), date(2026, 11, 15), 6000, 2000),
                period(date(2026, 11, 15), date(2026, 12, 15), 6200, 2000),
            ],
            adjustments: Vec::new(),
            transactions: vec![Transaction {
                date: date(2026, 11, 16),
                time: NaiveTime::from_hms_opt(14, 30, 0).unwrap(),
                amount_czk: 2530,
                original_amount: 100.0,
                currency: Currency::Eur,
                category: Category::Nutrition,
                kind: Kind::Expense,
                note: "lunch abroad".into(),
            }],
        };
        data.save_to(&path).unwrap();
        assert_eq!(FinanceData::load_from(&path).unwrap(), data);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn rejects_bad_values() {
        let dir = temp("bad");
        let path = dir.join(FILE);

        let bad_period = FinanceData {
            periods: vec![period(date(2026, 11, 15), date(2026, 11, 15), 0, 0)],
            ..FinanceData::default()
        };
        assert!(bad_period.save_to(&path).is_err());

        let bad_amount = FinanceData {
            adjustments: Vec::new(),
            transactions: vec![Transaction {
                date: date(2026, 11, 16),
                time: NaiveTime::from_hms_opt(12, 0, 0).unwrap(),
                amount_czk: 0,
                original_amount: 0.0,
                currency: Currency::Czk,
                category: Category::Allowance,
                kind: Kind::Expense,
                note: String::new(),
            }],
            ..FinanceData::default()
        };
        fs::write(&path, toml::to_string_pretty(&bad_amount).unwrap()).unwrap();
        assert!(FinanceData::load_from(&path).is_err());

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn migrates_v4_to_one_allowance_window() {
        let dir = temp("v4");
        let path = dir.join(FILE);
        fs::write(
            &path,
            r#"
version = 4
last_payday = "2026-11-01"
next_payday = "2026-12-01"

[[transactions]]
date = "2026-11-03"
time = "12:30:00"
amount_czk = 1000
original_amount = 1000.0
currency = "czk"
kind = "income"
note = "salary"

[[transactions]]
date = "2026-11-04"
time = "12:30:00"
amount_czk = 150
original_amount = 150.0
currency = "czk"
kind = "expense"
note = "lunch"
"#,
        )
        .unwrap();

        let data = FinanceData::load_from(&path).unwrap();
        assert_eq!(data.version, VERSION);
        assert_eq!(data.periods.len(), 1);
        assert_eq!(data.periods[0].budget(Category::Allowance), 0);
        assert_eq!(data.transactions.len(), 2);
        assert!(data.transactions.iter().any(|tx| tx.kind == Kind::Income));
        assert!(data.transactions.iter().all(|tx| tx.category == Category::Allowance));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn migrates_v1_dropping_savings() {
        let dir = temp("v1");
        let path = dir.join(FILE);
        fs::write(
            &path,
            r#"
version = 1
payday_day = 15
salary = 42000
initial_savings = 120000

[[transactions]]
date = "2026-11-16"
amount = 250
kind = "expense"
note = "groceries"

[[transactions]]
date = "2026-11-17"
amount = 5000
kind = "saving"
note = "save"
"#,
        )
        .unwrap();

        let data = FinanceData::load_from(&path).unwrap();
        assert_eq!(data.version, VERSION);
        assert_eq!(data.periods.len(), 1);
        assert_eq!(data.transactions.len(), 1);
        assert_eq!(data.transactions[0].currency, Currency::Czk);
        fs::remove_dir_all(&dir).unwrap();
    }
}
