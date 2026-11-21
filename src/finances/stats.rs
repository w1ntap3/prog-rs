//! Per-window, per-category math: actual money, the fixed average plan, the
//! adaptive break-even rate, projections, and the slideshow statements. Pure
//! functions over `FinanceData`, so the numbers are testable without a window.

use std::collections::BTreeMap;

use chrono::{NaiveDate, Timelike};

use super::model::{Category, FinanceData, Kind, Period};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Statement {
    /// Short accent line, usually a formatted amount.
    pub value: String,
    /// Sentence after the accent line.
    pub text: String,
}

#[derive(Debug, Clone)]
#[allow(dead_code, reason = "summary detail kept for future UI")]
pub struct Summary {
    pub category: Category,
    /// One-pot window; category is ignored and all transactions count.
    pub combined: bool,
    /// Last payday (first day of the window, inclusive).
    pub start: NaiveDate,
    /// The next payday (first day of the next window).
    pub next_payday: NaiveDate,
    pub total_days: i64,
    /// Days drawn so far: 0 before the window, full for past windows.
    pub day_index: i64,
    pub budget: i64,
    pub spent: i64,
    pub gained: i64,
    /// Net effect of manual balance corrections.
    pub adjusted: i64,
    /// Everything available for this window: balance + gains + corrections.
    pub available: i64,
    pub remaining: i64,

    /// Average plan: starts at the first income transaction of the window and
    /// slopes to zero at the next payday.
    pub plan_available: bool,
    pub plan_active: bool,
    pub plan_start_x: f32,
    pub plan_amount: i64,
    pub plan_now: i64,
    pub avg_daily: f64,
    /// `remaining - plan_now` while the plan is active.
    pub gap: i64,

    /// Adaptive break-even: what is left divided by the days still left.
    pub required_daily: f64,
    /// Average daily spending over the last up-to-7 days of the window.
    pub last_week_daily: Option<i64>,
    pub last_week_days: i64,
    /// Percent of this view's available money still left.
    pub left_percent: Option<i64>,

    /// Where the window ends if the current spending pace continues.
    pub projected_left: i64,
    pub days_ahead: i64,
    /// Spending in this category today (0 outside today).
    pub today_spent: i64,
    pub today_gained: i64,
    pub biggest: Option<(String, i64)>,
    pub is_current: bool,
    pub is_past: bool,
    pub is_future: bool,
    /// `(day-fraction, money left)`. Transactions are placed at their actual
    /// time, so gains and spending appear as vertical steps.
    pub points: Vec<(f32, i64)>,
}

impl Summary {
    pub fn compute(
        data: &FinanceData,
        period_index: usize,
        category: Category,
        today: NaiveDate,
    ) -> Self {
        let index = period_index.min(data.periods.len().saturating_sub(1));
        let period = &data.periods[index];
        let start = period.start;
        let next_payday = period.end;
        let total_days = period.days();
        let combined = period.combined;

        let is_future = today < start;
        let is_past = today >= next_payday;
        let is_current = !is_future && !is_past;
        let day_index = if is_future {
            0
        } else if is_past {
            total_days
        } else {
            ((today - start).num_days() + 1).clamp(1, total_days)
        };
        let cutoff = if is_future {
            start.pred_opt().unwrap_or(start)
        } else if is_past {
            next_payday.pred_opt().unwrap_or(start)
        } else {
            today
        };

        let budget = period.balance(category);
        let mut spent = 0;
        let mut gained = 0;
        let mut adjusted = 0;
        let mut today_spent = 0;
        let mut today_gained = 0;
        let mut events: Vec<(f32, i64)> = Vec::new();
        let mut per_note: BTreeMap<String, i64> = BTreeMap::new();
        let mut per_day_spent: BTreeMap<NaiveDate, i64> = BTreeMap::new();

        for tx in &data.transactions {
            if (!combined && tx.category != category)
                || tx.date < start
                || tx.date >= next_payday
                || tx.date > cutoff
            {
                continue;
            }
            match tx.kind {
                Kind::Expense => {
                    spent += tx.amount_czk;
                    events.push((transaction_x(start, tx.date, tx.time), -tx.amount_czk));
                    *per_day_spent.entry(tx.date).or_default() += tx.amount_czk;
                    if tx.date == today {
                        today_spent += tx.amount_czk;
                    }
                    let name = tx.note.trim();
                    let name = if name.is_empty() { "other" } else { name };
                    *per_note.entry(name.to_owned()).or_default() += tx.amount_czk;
                }
                Kind::Income => {
                    gained += tx.amount_czk;
                    events.push((transaction_x(start, tx.date, tx.time), tx.amount_czk));
                    if tx.date == today {
                        today_gained += tx.amount_czk;
                    }
                }
            }
        }

        for adj in &data.adjustments {
            if (!combined && adj.category != category)
                || adj.date < start
                || adj.date >= next_payday
                || adj.date > cutoff
            {
                continue;
            }
            adjusted += adj.delta_czk;
            events.push((transaction_x(start, adj.date, adj.time), adj.delta_czk));
        }

        let available = budget + gained + adjusted;
        let remaining = available - spent;

        // The average plan only exists when the window's first moment is an
        // income payout (all transactions at that moment must be income).
        let window_txs: Vec<&super::model::Transaction> = data
            .transactions
            .iter()
            .filter(|tx| {
                (combined || tx.category == category)
                    && tx.date >= start
                    && tx.date < next_payday
            })
            .collect();
        let first_key = window_txs.iter().map(|tx| (tx.date, tx.time)).min();
        let plan = first_key.and_then(|key| {
            let at_start: Vec<&super::model::Transaction> = window_txs
                .iter()
                .copied()
                .filter(|tx| (tx.date, tx.time) == key)
                .collect();
            if at_start.iter().all(|tx| tx.kind == Kind::Income) {
                Some((
                    transaction_x(start, key.0, key.1),
                    at_start.iter().map(|tx| tx.amount_czk).sum::<i64>(),
                ))
            } else {
                None
            }
        });
        let plan_available = plan.is_some();
        let (plan_start_x, plan_amount, plan_active, plan_now, avg_daily) = match plan {
            Some((x0, amount)) => {
                let span = (total_days as f32 - x0).max(f32::EPSILON);
                let active = day_index as f32 >= x0;
                let now = if active {
                    (amount as f64
                        * ((total_days as f32 - day_index as f32) / span) as f64)
                        .round() as i64
                } else {
                    0
                };
                (x0, amount, active, now, amount as f64 / span as f64)
            }
            None => (0.0, 0, false, 0, 0.0),
        };

        let gap = if plan_active { remaining - plan_now } else { 0 };
        let days_ahead = if plan_active && avg_daily > 0.0 {
            (gap as f64 / avg_daily).round() as i64
        } else {
            0
        };

        let days_left = if is_future {
            total_days
        } else if is_past {
            0
        } else {
            (total_days - day_index + 1).max(1)
        };
        let required_daily = if days_left > 0 {
            remaining as f64 / days_left as f64
        } else {
            0.0
        };

        let last_day = if is_future {
            0
        } else if is_past {
            total_days
        } else {
            day_index
        };
        let last_week_days = last_day.min(7);
        let last_week_daily = if last_week_days > 0 {
            let sum: i64 = (0..last_week_days)
                .map(|offset| {
                    let date = start + chrono::Duration::days(last_day - last_week_days + offset);
                    per_day_spent.get(&date).copied().unwrap_or(0)
                })
                .sum();
            Some((sum as f64 / last_week_days as f64).round() as i64)
        } else {
            None
        };

        let left_percent = if combined {
            if available > 0 {
                Some(((remaining * 100) / available).clamp(0, 100))
            } else {
                None
            }
        } else {
            category_percent(data, period, category, cutoff)
        };

        let projected_left = if day_index > 0 {
            available - (spent as f64 / day_index as f64 * total_days as f64).round() as i64
        } else {
            available
        };

        events.sort_by(|a, b| a.0.total_cmp(&b.0));
        let total_f = total_days as f32;
        let mut money = budget;
        let mut points = vec![(0.0f32, budget)];
        let mut last_x = 0.0f32;
        for (x, delta) in events {
            if x > last_x {
                points.push((x, money));
                last_x = x;
            }
            money += delta;
            points.push((x, money));
        }

        let end_x = if is_future {
            0.0
        } else if is_past {
            total_f
        } else {
            day_index as f32
        };
        if end_x > last_x {
            points.push((end_x, money));
        }

        let biggest = per_note.into_iter().max_by_key(|(_, amount)| *amount);

        Self {
            category,
            combined,
            start,
            next_payday,
            total_days,
            day_index,
            budget,
            spent,
            gained,
            adjusted,
            available,
            remaining,
            plan_available,
            plan_active,
            plan_start_x,
            plan_amount,
            plan_now,
            avg_daily,
            gap,
            required_daily,
            last_week_daily,
            last_week_days,
            left_percent,
            projected_left,
            days_ahead,
            today_spent,
            today_gained,
            biggest,
            is_current,
            is_past,
            is_future,
            points,
        }
    }

    pub fn threshold_round(&self) -> i64 {
        self.required_daily.round() as i64
    }

    pub fn statements(&self) -> Vec<Statement> {
        let mut out = Vec::new();
        let category = if self.combined {
            "combined"
        } else {
            self.category.label()
        };

        if self.available == 0 {
            out.push(Statement {
                value: "—".into(),
                text: format!("set the {category} balance"),
            });
        }

        if !self.plan_available {
            out.push(Statement {
                value: "—".into(),
                text: "average plan unavailable · first transaction must be income".into(),
            });
        } else if !self.plan_active {
            out.push(Statement {
                value: "—".into(),
                text: "the average plan hasn't started yet".into(),
            });
        } else {
            let value = if self.gap >= 0 {
                format!("+{}", money(self.gap))
            } else {
                format!("-{}", money(-self.gap))
            };
            out.push(Statement {
                value,
                text: format!("from the {category} average plan"),
            });
        }

        if let Some(average) = self.last_week_daily {
            out.push(Statement {
                value: money(average),
                text: format!(
                    "average per day over the last {} {}",
                    self.last_week_days,
                    if self.last_week_days == 1 { "day" } else { "days" }
                ),
            });
        }

        if let Some(percent) = self.left_percent {
            let label = if self.combined {
                "combined"
            } else {
                self.category.label()
            };
            out.push(Statement {
                value: format!("{percent}%"),
                text: format!("of {label} left"),
            });
        }

        out
    }
}

/// Percentage of a category's available money still left at `cutoff`.
pub fn category_percent(
    data: &FinanceData,
    period: &Period,
    category: Category,
    cutoff: NaiveDate,
) -> Option<i64> {
    let mut available = period.budget(category);
    let mut spent = 0;
    for tx in &data.transactions {
        if tx.category != category
            || tx.date < period.start
            || tx.date >= period.end
            || tx.date > cutoff
        {
            continue;
        }
        match tx.kind {
            Kind::Expense => spent += tx.amount_czk,
            Kind::Income => available += tx.amount_czk,
        }
    }
    for adj in &data.adjustments {
        if adj.category != category
            || adj.date < period.start
            || adj.date >= period.end
            || adj.date > cutoff
        {
            continue;
        }
        available += adj.delta_czk;
    }
    if available <= 0 {
        return None;
    }
    Some(((available - spent) * 100 / available).clamp(0, 100))
}

fn transaction_x(start: NaiveDate, date: NaiveDate, time: chrono::NaiveTime) -> f32 {
    let days = (date - start).num_days() as f32;
    let seconds = time.num_seconds_from_midnight() as f32;
    days + seconds / 86_400.0
}

/// `12 345 CZK`, with spaces grouping thousands.
pub fn money(amount: i64) -> String {
    let digits = amount.abs().to_string();
    let mut grouped = String::new();
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            grouped.push(' ');
        }
        grouped.push(ch);
    }
    let sign = if amount < 0 { "-" } else { "" };
    format!("{sign}{grouped} CZK")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::currency::Currency;
    use crate::finances::model::{Period, Transaction};
    use chrono::NaiveTime;

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    fn tx(date: NaiveDate, amount: i64, category: Category, note: &str) -> Transaction {
        Transaction {
            date,
            time: NaiveTime::from_hms_opt(12, 0, 0).unwrap(),
            amount_czk: amount,
            original_amount: amount as f64,
            currency: Currency::Czk,
            category,
            kind: Kind::Expense,
            note: note.into(),
        }
    }

    fn gain(date: NaiveDate, amount: i64, category: Category) -> Transaction {
        Transaction {
            date,
            time: NaiveTime::from_hms_opt(12, 0, 0).unwrap(),
            amount_czk: amount,
            original_amount: amount as f64,
            currency: Currency::Czk,
            category,
            kind: Kind::Income,
            note: "top up".into(),
        }
    }

    fn data() -> FinanceData {
        let mut period = Period {
            start: date(2026, 11, 1),
            end: date(2026, 12, 1),
            combined: false,
            combined_budget: None,
            budgets: Vec::new(),
        };
        period.set_budget(Category::Nutrition, 3000);
        period.set_budget(Category::Allowance, 1000);
        FinanceData {
            version: 6,
            periods: vec![period],
            transactions: vec![
                tx(date(2026, 11, 3), 100, Category::Nutrition, "food"),
                tx(date(2026, 11, 4), 200, Category::Allowance, "fun"),
            ],
            adjustments: Vec::new(),
        }
    }

    #[test]
    fn adaptive_daily_is_remaining_over_days_left() {
        let data = data();
        let summary = Summary::compute(&data, 0, Category::Nutrition, date(2026, 11, 5));
        assert_eq!(summary.total_days, 30);
        assert_eq!(summary.budget, 3000);
        assert_eq!(summary.spent, 100);
        assert_eq!(summary.remaining, 2900);
        // 26 days left including today.
        assert!((summary.required_daily - 2900.0 / 26.0).abs() < 0.001);
    }

    #[test]
    fn categories_do_not_mix() {
        let data = data();
        let nutrition = Summary::compute(&data, 0, Category::Nutrition, date(2026, 11, 5));
        let allowance = Summary::compute(&data, 0, Category::Allowance, date(2026, 11, 5));
        assert_eq!(nutrition.spent, 100);
        assert_eq!(allowance.spent, 200);
        assert_eq!(nutrition.today_spent, 0);
    }

    #[test]
    fn past_window_shows_full_curve() {
        let data = data();
        let summary = Summary::compute(&data, 0, Category::Nutrition, date(2026, 12, 2));
        assert!(summary.is_past);
        assert_eq!(summary.day_index, summary.total_days);
        assert!((summary.points.last().unwrap().0 - summary.total_days as f32).abs() < f32::EPSILON);
    }

    #[test]
    fn future_window_starts_at_zero() {
        let data = data();
        let summary = Summary::compute(&data, 0, Category::Nutrition, date(2026, 10, 20));
        assert!(summary.is_future);
        assert_eq!(summary.day_index, 0);
        assert_eq!(summary.spent, 0);
        assert_eq!(summary.points.len(), 1);
    }

    #[test]
    fn income_adds_to_remaining() {
        let mut data = data();
        data.transactions
            .push(gain(date(2026, 11, 3), 500, Category::Nutrition));
        let summary = Summary::compute(&data, 0, Category::Nutrition, date(2026, 11, 5));
        assert_eq!(summary.spent, 100);
        assert_eq!(summary.gained, 500);
        assert_eq!(summary.available, 3500);
        assert_eq!(summary.remaining, 3400);
        assert_eq!(summary.points.last().unwrap().1, 3400);
    }

    #[test]
    fn gains_are_vertical_steps_at_their_time() {
        let mut data = data();
        data.transactions.clear();
        data.periods[0].set_budget(Category::Nutrition, 0);
        data.transactions
            .push(gain(date(2026, 11, 1), 3000, Category::Nutrition));
        let summary = Summary::compute(&data, 0, Category::Nutrition, date(2026, 11, 5));
        let step = summary.points.windows(2).any(|pair| {
            (pair[0].0 - pair[1].0).abs() < f32::EPSILON
                && pair[0].1 == 0
                && pair[1].1 == 3000
        });
        assert!(step, "expected a vertical jump at the gain time: {:?}", summary.points);
    }

    #[test]
    fn plan_requires_income_as_first_transaction() {
        let data = data();
        let summary = Summary::compute(&data, 0, Category::Nutrition, date(2026, 11, 5));
        assert!(!summary.plan_available);
        assert!(summary.statements().iter().any(|s| s.text.contains("average plan unavailable")));
    }

    #[test]
    fn plan_starts_at_first_income() {
        let mut data = data();
        data.transactions.clear();
        data.transactions
            .push(gain(date(2026, 11, 1), 3000, Category::Nutrition));
        let summary = Summary::compute(&data, 0, Category::Nutrition, date(2026, 11, 5));
        assert!(summary.plan_available);
        assert!(summary.plan_active);
        assert_eq!(summary.plan_amount, 3000);
        assert!(summary.plan_now > 0);
        // At the end of the window the plan is exactly zero.
        let end = Summary::compute(&data, 0, Category::Nutrition, date(2026, 12, 2));
        assert_eq!(end.plan_now, 0);
    }

    #[test]
    fn week_average_and_left_percent() {
        let data = data();
        let summary = Summary::compute(&data, 0, Category::Nutrition, date(2026, 11, 5));
        assert_eq!(summary.last_week_days, 5);
        assert_eq!(summary.last_week_daily, Some(20));
        assert_eq!(summary.left_percent, Some(96));
    }

    #[test]
    fn combined_window_ignores_categories_and_uses_one_balance() {
        let mut period = Period {
            start: date(2026, 11, 1),
            end: date(2026, 12, 1),
            combined: true,
            combined_budget: Some(3000),
            budgets: Vec::new(),
        };
        period.set_budget(Category::Nutrition, 1000);
        period.set_budget(Category::Allowance, 500);
        let data = FinanceData {
            version: 6,
            periods: vec![period],
            transactions: vec![
                tx(date(2026, 11, 3), 100, Category::Nutrition, "food"),
                tx(date(2026, 11, 4), 200, Category::Allowance, "fun"),
            ],
            adjustments: Vec::new(),
        };
        let summary = Summary::compute(&data, 0, Category::Nutrition, date(2026, 11, 5));
        assert!(summary.combined);
        assert_eq!(summary.budget, 3000);
        assert_eq!(summary.spent, 300);
        assert_eq!(summary.remaining, 2700);
        assert_eq!(summary.left_percent, Some(90));
        assert!(
            summary
                .statements()
                .iter()
                .any(|statement| statement.text == "of combined left")
        );
    }

    #[test]
    fn money_groups_thousands() {
        assert_eq!(money(0), "0 CZK");
        assert_eq!(money(1234), "1 234 CZK");
        assert_eq!(money(-1234567), "-1 234 567 CZK");
    }
}
