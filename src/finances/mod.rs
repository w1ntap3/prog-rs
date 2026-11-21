//! The `finances` section: category tabs, a per-window baseline chart, and
//! data entry. Everything numeric comes from `stats::Summary`; this module only
//! owns the UI state and the live `FinanceData`.

mod graph;
mod model;
mod stats;

use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use chrono::{Datelike, Local, NaiveDate, NaiveTime, Timelike};
use gpui::prelude::*;
use gpui::{
    Animation, AnimationExt, AnyElement, Bounds, ClickEvent, Context, FocusHandle, FontWeight,
    KeyDownEvent, MouseButton, MouseMoveEvent, Point, Render, Window, anchored, deferred, div,
    point, px, rgb,
};

use crate::currency::{Currency, Rates, parse_decimal, trim_decimal};
use crate::theme;

use self::model::{
    Adjustment, FinanceData, Kind, Period, Transaction, add_months, days_in_month, first_of_month,
};
pub use self::model::Category;
use self::stats::Summary;
pub use self::stats::{Statement, money};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Field {
    Amount,
    Note,
    AdjustBalance,
    NutritionBudget,
    AllowanceBudget,
    CombinedBudget,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DateTarget {
    Transaction,
    Adjustment,
    PeriodStart,
    PeriodEnd,
    NewStart,
    NewEnd,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TimeTarget {
    Transaction,
    Adjustment,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Picker {
    Date(DateTarget),
    Time(TimeTarget),
}

struct FocusHandles {
    amount: FocusHandle,
    note: FocusHandle,
    adjust_balance: FocusHandle,
    nutrition_budget: FocusHandle,
    allowance_budget: FocusHandle,
    combined_budget: FocusHandle,
}

/// Read-only finance headline used by the `main` tab.
pub struct HomeSnapshot {
    pub statements: Vec<Statement>,
    pub daily: Option<i64>,
}

pub struct Finances {
    data: FinanceData,
    rates: Rates,
    entry_currency: Currency,
    entry_category: Category,
    entry_kind: Kind,
    adjust_currency: Currency,
    amount: String,
    note: String,
    adjust_balance: String,
    nutrition_budget: String,
    allowance_budget: String,
    combined_budget: String,
    date: NaiveDate,
    time: NaiveTime,
    adjust_date: NaiveDate,
    adjust_time: NaiveTime,
    selected_period: usize,
    selected_category: Category,
    /// Draft dates for a window that hasn't been inserted yet.
    new_window: Option<(NaiveDate, NaiveDate)>,
    picker: Option<Picker>,
    /// Window-space position where the picker was opened; the panel is clamped
    /// to the window around this point.
    picker_anchor: Point<gpui::Pixels>,
    /// First day of the month shown in the calendar popover.
    cal_month: NaiveDate,
    slide: usize,
    status: Option<String>,
    chart_bounds: Rc<Cell<Bounds<gpui::Pixels>>>,
    hover_position: Option<Point<gpui::Pixels>>,
    focus: FocusHandles,
}

impl Finances {
    pub fn new(rates: Rates, cx: &mut Context<Self>) -> Self {
        let (data, status) = match FinanceData::load_or_create() {
            Ok(data) => (data, None),
            Err(error) => (
                FinanceData::default(),
                Some(format!("could not load finances: {error:#}")),
            ),
        };

        let now = Local::now();
        let today = now.date_naive();
        let time = NaiveTime::from_hms_opt(now.hour(), now.minute(), 0)
            .unwrap_or_else(|| NaiveTime::from_hms_opt(12, 0, 0).expect("noon is valid"));
        let selected_period = data.current_period_index(today);
        let period = &data.periods[selected_period];
        let this = Self {
            nutrition_budget: amount_input(period.budget(Category::Nutrition)),
            allowance_budget: amount_input(period.budget(Category::Allowance)),
            combined_budget: amount_input(period.combined_balance()),
            date: today,
            time,
            adjust_date: today,
            adjust_time: time,
            cal_month: first_of_month(today),
            picker: None,
            picker_anchor: point(px(0.), px(0.)),
            selected_period,
            selected_category: Category::Nutrition,
            new_window: None,
            rates,
            data,
            entry_currency: Currency::Czk,
            entry_category: Category::Nutrition,
            entry_kind: Kind::Expense,
            adjust_currency: Currency::Czk,
            amount: String::new(),
            note: String::new(),
            adjust_balance: String::new(),
            slide: 0,
            status,
            chart_bounds: Rc::new(Cell::new(Bounds::default())),
            hover_position: None,
            focus: FocusHandles {
                amount: cx.focus_handle(),
                note: cx.focus_handle(),
                adjust_balance: cx.focus_handle(),
                nutrition_budget: cx.focus_handle(),
                allowance_budget: cx.focus_handle(),
                combined_budget: cx.focus_handle(),
            },
        };
        this
    }

    /// Summary shown on the `main` tab. `None` = combined, otherwise the
    /// selected category of the same window.
    pub fn home_snapshot(&self, category: Option<Category>) -> HomeSnapshot {
        if self.data.periods.is_empty() {
            return HomeSnapshot {
                statements: Vec::new(),
                daily: None,
            };
        }
        let today = Local::now().date_naive();
        let index = self.data.current_period_index(today);
        let period = &self.data.periods[index];

        let summary = match category {
            None => {
                if period.combined {
                    Summary::compute(&self.data, index, Category::Nutrition, today)
                } else {
                    let mut data = self.data.clone();
                    let total = data.periods[index].total_budget();
                    let period = &mut data.periods[index];
                    period.combined = true;
                    period.set_combined_balance(total);
                    Summary::compute(&data, index, Category::Nutrition, today)
                }
            }
            Some(category) => {
                let mut data = self.data.clone();
                data.periods[index].combined = false;
                Summary::compute(&data, index, category, today)
            }
        };

        HomeSnapshot {
            statements: summary.statements(),
            daily: if summary.is_past || summary.available == 0 {
                None
            } else {
                Some(summary.threshold_round())
            },
        }
    }

    /// Called by the settings tab when conversion rates change.
    pub fn set_rates(&mut self, rates: Rates, cx: &mut Context<Self>) {
        self.rates = rates;
        cx.notify();
    }

    // ---- state plumbing -------------------------------------------------

    fn period(&self) -> &Period {
        &self.data.periods[self.selected_period]
    }

    fn value(&self, field: Field) -> &str {
        match field {
            Field::Amount => &self.amount,
            Field::Note => &self.note,
            Field::AdjustBalance => &self.adjust_balance,
            Field::NutritionBudget => &self.nutrition_budget,
            Field::AllowanceBudget => &self.allowance_budget,
            Field::CombinedBudget => &self.combined_budget,
        }
    }

    fn value_mut(&mut self, field: Field) -> &mut String {
        match field {
            Field::Amount => &mut self.amount,
            Field::Note => &mut self.note,
            Field::AdjustBalance => &mut self.adjust_balance,
            Field::NutritionBudget => &mut self.nutrition_budget,
            Field::AllowanceBudget => &mut self.allowance_budget,
            Field::CombinedBudget => &mut self.combined_budget,
        }
    }

    fn focus_handle(&self, field: Field) -> &FocusHandle {
        match field {
            Field::Amount => &self.focus.amount,
            Field::Note => &self.focus.note,
            Field::AdjustBalance => &self.focus.adjust_balance,
            Field::NutritionBudget => &self.focus.nutrition_budget,
            Field::AllowanceBudget => &self.focus.allowance_budget,
            Field::CombinedBudget => &self.focus.combined_budget,
        }
    }

    fn focus_field(&mut self, field: Field, window: &mut Window, cx: &mut Context<Self>) {
        self.focus_handle(field).focus(window, cx);
        cx.notify();
    }

    fn key(
        &mut self,
        field: Field,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let modifiers = event.keystroke.modifiers;
        let key = event.keystroke.key.as_str();

        if key == "tab" {
            let next = if self.period().combined {
                match field {
                    Field::Amount => Field::Note,
                    Field::Note => Field::AdjustBalance,
                    Field::AdjustBalance => Field::CombinedBudget,
                    Field::CombinedBudget => Field::Amount,
                    _ => Field::Amount,
                }
            } else {
                match field {
                    Field::Amount => Field::Note,
                    Field::Note => Field::AdjustBalance,
                    Field::AdjustBalance => Field::NutritionBudget,
                    Field::NutritionBudget => Field::AllowanceBudget,
                    Field::AllowanceBudget | Field::CombinedBudget => Field::Amount,
                }
            };
            self.focus_field(next, window, cx);
            return;
        }
        if key == "enter" {
            match field {
                Field::Amount | Field::Note => self.add_transaction(cx),
                Field::AdjustBalance => self.apply_correction(cx),
                Field::NutritionBudget => self.apply_budget(Category::Nutrition, cx),
                Field::AllowanceBudget => self.apply_budget(Category::Allowance, cx),
                Field::CombinedBudget => self.apply_combined_budget(cx),
            }
            return;
        }
        if key == "backspace" {
            let value = self.value_mut(field);
            if modifiers.control || modifiers.platform {
                value.clear();
            } else {
                value.pop();
            }
            cx.notify();
            return;
        }
        if modifiers.control || modifiers.alt || modifiers.platform {
            return;
        }
        let Some(chars) = event.keystroke.key_char.as_deref() else {
            return;
        };
        let decimal = matches!(
            field,
            Field::Amount
                | Field::AdjustBalance
                | Field::NutritionBudget
                | Field::AllowanceBudget
                | Field::CombinedBudget
        );
        let accepted = if decimal {
            chars.chars().all(|c| {
                c.is_ascii_digit()
                    || c == '.'
                    || c == ','
                    || (field == Field::AdjustBalance && c == '-')
            })
        } else if field == Field::Note {
            true
        } else {
            chars.chars().all(|c| c.is_ascii_digit())
        };
        if !accepted {
            return;
        }
        if decimal {
            let adds_separator = chars.chars().any(|c| c == '.' || c == ',');
            let has_separator = self.value(field).contains(['.', ',']);
            if adds_separator && has_separator {
                return;
            }
            if chars.contains('-') && !self.value(field).is_empty() {
                return;
            }
        }
        self.value_mut(field).push_str(chars);
        cx.notify();
    }

    /// Save and report a failure in the status line. Returns success.
    fn persist(&mut self) -> bool {
        match self.data.save() {
            Ok(()) => true,
            Err(error) => {
                self.status = Some(format!("save failed: {error:#}"));
                false
            }
        }
    }

    fn add_transaction(&mut self, cx: &mut Context<Self>) {
        let Some(amount) = parse_decimal(&self.amount) else {
            self.status = Some("enter an amount first".into());
            cx.notify();
            return;
        };
        if amount <= 0.0 {
            self.status = Some("amount must be greater than zero".into());
            cx.notify();
            return;
        }

        let rate = self.rates.czk_per(self.entry_currency);
        if rate <= 0.0 {
            self.status = Some(format!(
                "set the {} -> CZK rate in settings first",
                self.entry_currency.label()
            ));
            cx.notify();
            return;
        }
        let amount_czk = (amount * rate * 100.0).round() as i64;
        if amount_czk <= 0 {
            self.status = Some("amount is too small to be worth 1 CZK".into());
            cx.notify();
            return;
        }

        self.data.transactions.push(Transaction {
            date: self.date,
            time: self.time,
            amount_czk,
            original_amount: amount,
            currency: self.entry_currency,
            category: self.entry_category,
            kind: self.entry_kind,
            note: self.note.trim().to_owned(),
        });
        let entered = if self.entry_currency == Currency::Czk {
            money(amount_czk)
        } else {
            format!(
                "{} {} = {}",
                trim_decimal(amount),
                self.entry_currency.label(),
                money(amount_czk)
            )
        };
        self.amount.clear();
        self.note.clear();
        self.picker = None;
        if self.persist() {
            self.status = Some(format!(
                "added {} · {} · {entered}",
                self.entry_kind.verb(),
                self.entry_category.label()
            ));
        }
        cx.notify();
    }

    fn delete_transaction(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.data.transactions.len() {
            self.data.transactions.remove(index);
            if self.persist() {
                self.status = Some("transaction deleted".into());
            }
            cx.notify();
        }
    }

    fn balance_at(&self, at_date: NaiveDate, at_time: NaiveTime) -> i64 {
        let period = self.period();
        let combined = period.combined;
        let category = self.selected_category;
        let at = (at_date, at_time);
        let mut balance = period.balance(category);

        for tx in &self.data.transactions {
            if (!combined && tx.category != category)
                || tx.date < period.start
                || tx.date >= period.end
                || (tx.date, tx.time) > at
            {
                continue;
            }
            match tx.kind {
                Kind::Expense => balance -= tx.amount_czk,
                Kind::Income => balance += tx.amount_czk,
            }
        }
        for adj in &self.data.adjustments {
            if (!combined && adj.category != category)
                || adj.date < period.start
                || adj.date >= period.end
                || (adj.date, adj.time) > at
            {
                continue;
            }
            balance += adj.delta_czk;
        }
        balance
    }

    fn apply_correction(&mut self, cx: &mut Context<Self>) {
        let Some(target) = parse_decimal(&self.adjust_balance) else {
            self.status = Some("enter the real balance in CZK".into());
            cx.notify();
            return;
        };
        let rate = self.rates.czk_per(self.adjust_currency);
        if rate <= 0.0 {
            self.status = Some(format!(
                "set the {} -> CZK rate in settings first",
                self.adjust_currency.label()
            ));
            cx.notify();
            return;
        }
        let target = (target * rate * 100.0).round() as i64;
        let (start, end) = {
            let period = self.period();
            (period.start, period.end)
        };
        if self.adjust_date < start || self.adjust_date >= end {
            self.status = Some("the correction must happen inside this window".into());
            cx.notify();
            return;
        }

        let current = self.balance_at(self.adjust_date, self.adjust_time);
        let delta = target - current;
        if delta == 0 {
            self.status = Some("the balance already matches".into());
            cx.notify();
            return;
        }

        self.data.adjustments.push(Adjustment {
            date: self.adjust_date,
            time: self.adjust_time,
            delta_czk: delta,
            category: self.selected_category,
        });
        self.adjust_balance.clear();
        let signed = if delta > 0 {
            format!("+{}", money(delta))
        } else {
            format!("-{}", money(-delta))
        };
        if self.persist() {
            self.status = Some(format!("balance corrected by {signed}"));
        }
        cx.notify();
    }

    fn delete_adjustment(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.data.adjustments.len() {
            self.data.adjustments.remove(index);
            if self.persist() {
                self.status = Some("correction removed".into());
            }
            cx.notify();
        }
    }

    fn apply_budget(&mut self, category: Category, cx: &mut Context<Self>) {
        let raw = match category {
            Category::Nutrition => self.nutrition_budget.clone(),
            Category::Allowance => self.allowance_budget.clone(),
        };
        let Some(amount) = parse_decimal(&raw) else {
            self.status = Some(format!("enter the {} balance in CZK", category.label()));
            cx.notify();
            return;
        };
        if amount < 0.0 {
            self.status = Some("balance must not be negative".into());
            cx.notify();
            return;
        }
        let amount = (amount * 100.0).round() as i64;
        let index = self.selected_period;
        self.data.periods[index].set_budget(category, amount);
        match category {
            Category::Nutrition => self.nutrition_budget = amount_input(amount),
            Category::Allowance => self.allowance_budget = amount_input(amount),
        }
        if self.persist() {
            self.status = Some(format!(
                "{} balance set to {}",
                category.label(),
                money(amount)
            ));
        }
        cx.notify();
    }

    fn apply_combined_budget(&mut self, cx: &mut Context<Self>) {
        let Some(amount) = parse_decimal(&self.combined_budget) else {
            self.status = Some("enter the window balance in CZK".into());
            cx.notify();
            return;
        };
        if amount < 0.0 {
            self.status = Some("balance must not be negative".into());
            cx.notify();
            return;
        }
        let amount = (amount * 100.0).round() as i64;
        let index = self.selected_period;
        self.data.periods[index].set_combined_balance(amount);
        self.combined_budget = amount_input(amount);
        if self.persist() {
            self.status = Some(format!("window balance set to {}", money(amount)));
        }
        cx.notify();
    }

    fn toggle_combined(&mut self, cx: &mut Context<Self>) {
        let index = self.selected_period;
        let period = &mut self.data.periods[index];
        if period.combined {
            period.combined = false;
            self.status = Some("window split by category again".into());
        } else {
            let total = period.total_budget();
            period.combined = true;
            period.set_combined_balance(total);
            self.status = Some("window marked as combined".into());
        }
        self.sync_period_fields();
        self.persist();
        cx.notify();
    }

    fn sync_period_fields(&mut self) {
        let (start, end, nutrition, allowance, combined) = {
            let period = &self.data.periods[self.selected_period];
            (
                period.start,
                period.end,
                period.budget(Category::Nutrition),
                period.budget(Category::Allowance),
                period.combined_balance(),
            )
        };
        self.nutrition_budget = amount_input(nutrition);
        self.allowance_budget = amount_input(allowance);
        self.combined_budget = amount_input(combined);

        // Keep the entry date inside the window being viewed, so adding while
        // browsing history lands in that window.
        let today = Local::now().date_naive();
        let clamped = if today < start {
            start
        } else if today >= end {
            end.pred_opt().unwrap_or(start)
        } else {
            today
        };
        if self.date < start || self.date >= end {
            self.date = clamped;
        }
        if self.adjust_date < start || self.adjust_date >= end {
            self.adjust_date = clamped;
        }
    }

    fn move_period(&mut self, delta: i32, cx: &mut Context<Self>) {
        let len = self.data.periods.len();
        let next = (self.selected_period as i32 + delta).clamp(0, len as i32 - 1) as usize;
        if next != self.selected_period {
            self.selected_period = next;
            self.sync_period_fields();
            self.picker = None;
            self.slide = 0;
            cx.notify();
        }
    }

    fn overlaps(&self, start: NaiveDate, end: NaiveDate, ignore: Option<usize>) -> bool {
        self.data
            .periods
            .iter()
            .enumerate()
            .any(|(index, period)| {
                Some(index) != ignore && period.start < end && start < period.end
            })
    }

    fn reorder_selected(&mut self, edited: Period) {
        self.data.periods.sort_by_key(|period| period.start);
        if let Some(index) = self
            .data
            .periods
            .iter()
            .position(|period| *period == edited)
        {
            self.selected_period = index;
        }
        self.sync_period_fields();
    }

    fn open_new_window(&mut self, cx: &mut Context<Self>) {
        if self.new_window.is_some() {
            self.new_window = None;
            cx.notify();
            return;
        }
        let last = self.data.periods.last().expect("at least one window");
        let start = last.end;
        self.new_window = Some((start, add_months(start, 1)));
        cx.notify();
    }

    fn create_window(&mut self, cx: &mut Context<Self>) {
        let Some((start, end)) = self.new_window else {
            return;
        };
        if start >= end {
            self.status = Some("window start must be before its end".into());
            cx.notify();
            return;
        }
        if self.overlaps(start, end, None) {
            self.status = Some("this window overlaps an existing one".into());
            cx.notify();
            return;
        }

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
        self.data.periods.push(period.clone());
        self.data.periods.sort_by_key(|period| period.start);
        self.selected_period = self
            .data
            .periods
            .iter()
            .position(|candidate| *candidate == period)
            .unwrap_or(0);
        self.new_window = None;
        self.picker = None;
        self.sync_period_fields();
        if self.persist() {
            self.status = Some("window added · set its balances above".into());
        }
        cx.notify();
    }

    fn date_for(&self, target: DateTarget) -> NaiveDate {
        let period = self.period();
        match target {
            DateTarget::Transaction => self.date,
            DateTarget::Adjustment => self.adjust_date,
            DateTarget::PeriodStart => period.start,
            DateTarget::PeriodEnd => period.end,
            DateTarget::NewStart => self.new_window.map(|(start, _)| start).unwrap_or(self.date),
            DateTarget::NewEnd => self.new_window.map(|(_, end)| end).unwrap_or(self.date),
        }
    }

    fn toggle_date_picker_at(
        &mut self,
        target: DateTarget,
        position: Point<gpui::Pixels>,
        cx: &mut Context<Self>,
    ) {
        let date = self.date_for(target);
        self.cal_month = first_of_month(date);
        self.picker_anchor = position;
        self.picker = if self.picker == Some(Picker::Date(target)) {
            None
        } else {
            Some(Picker::Date(target))
        };
        cx.notify();
    }

    fn toggle_time_picker(
        &mut self,
        target: TimeTarget,
        position: Point<gpui::Pixels>,
        cx: &mut Context<Self>,
    ) {
        self.picker_anchor = position;
        self.picker = if self.picker == Some(Picker::Time(target)) {
            None
        } else {
            Some(Picker::Time(target))
        };
        cx.notify();
    }

    fn choose_date(&mut self, target: DateTarget, date: NaiveDate, cx: &mut Context<Self>) {
        match target {
            DateTarget::Transaction => {
                self.date = date;
                self.picker = None;
            }
            DateTarget::Adjustment => {
                self.adjust_date = date;
                self.picker = None;
            }
            DateTarget::PeriodStart => {
                let index = self.selected_period;
                let end = self.data.periods[index].end;
                if date >= end {
                    self.status = Some("window start must be before its end".into());
                } else if self.overlaps(date, end, Some(index)) {
                    self.status = Some("this would overlap another window".into());
                } else {
                    self.data.periods[index].start = date;
                    let edited = self.data.periods[index].clone();
                    self.picker = None;
                    self.reorder_selected(edited);
                    if self.persist() {
                        self.status = Some("window saved".into());
                    }
                }
            }
            DateTarget::PeriodEnd => {
                let index = self.selected_period;
                let start = self.data.periods[index].start;
                if date <= start {
                    self.status = Some("window end must be after its start".into());
                } else if self.overlaps(start, date, Some(index)) {
                    self.status = Some("this would overlap another window".into());
                } else {
                    self.data.periods[index].end = date;
                    let edited = self.data.periods[index].clone();
                    self.picker = None;
                    self.reorder_selected(edited);
                    if self.persist() {
                        self.status = Some("window saved".into());
                    }
                }
            }
            DateTarget::NewStart => {
                if let Some((start, _)) = self.new_window.as_mut() {
                    *start = date;
                }
                self.picker = None;
            }
            DateTarget::NewEnd => {
                if let Some((_, end)) = self.new_window.as_mut() {
                    *end = date;
                }
                self.picker = None;
            }
        }
        self.cal_month = first_of_month(date);
        cx.notify();
    }

    fn shift_calendar(&mut self, delta: i32, cx: &mut Context<Self>) {
        self.cal_month = add_months(self.cal_month, delta);
        cx.notify();
    }

    fn set_now(&mut self, target: TimeTarget, cx: &mut Context<Self>) {
        let now = Local::now();
        let date = now.date_naive();
        let time = NaiveTime::from_hms_opt(now.hour(), now.minute(), 0)
            .unwrap_or_else(|| NaiveTime::from_hms_opt(12, 0, 0).expect("noon is valid"));
        match target {
            TimeTarget::Transaction => {
                self.date = date;
                self.time = time;
            }
            TimeTarget::Adjustment => {
                self.adjust_date = date;
                self.adjust_time = time;
            }
        }
        self.picker = None;
        cx.notify();
    }

    fn now_button(target: TimeTarget, cx: &Context<Self>) -> impl IntoElement {
        div()
            .id(format!("fin-now-{target:?}"))
            .pb(px(theme::space::FIELD_Y))
            .text_size(px(theme::type_scale::MINI))
            .text_color(rgb(theme::color::MUTED))
            .cursor_pointer()
            .hover(|style| style.text_color(rgb(theme::color::ACCENT)))
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.set_now(target, cx)
            }))
            .child("get current time")
    }

    fn time_for(&self, target: TimeTarget) -> NaiveTime {
        match target {
            TimeTarget::Transaction => self.time,
            TimeTarget::Adjustment => self.adjust_time,
        }
    }

    fn choose_time(
        &mut self,
        target: TimeTarget,
        time: NaiveTime,
        cx: &mut Context<Self>,
    ) {
        match target {
            TimeTarget::Transaction => self.time = time,
            TimeTarget::Adjustment => self.adjust_time = time,
        }
        cx.notify();
    }

    // ---- rendering helpers ---------------------------------------------

    fn kicker(text: &'static str) -> impl IntoElement {
        div()
            .text_size(px(theme::type_scale::KICKER))
            .text_color(rgb(theme::color::SEA_GREEN))
            .child(text.to_uppercase())
    }

    fn field(
        &self,
        field: Field,
        placeholder: &str,
        window: &Window,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let focus = self.focus_handle(field);
        let focused = focus.is_focused(window);
        let focus = focus.clone();
        let value = self.value(field).to_owned();
        let empty = value.is_empty();
        div()
            .id(format!("fin-field-{field:?}"))
            .w_full()
            .border_1()
            .border_color(rgb(if focused {
                theme::color::ACCENT
            } else {
                theme::color::SURFACE
            }))
            .bg(rgb(theme::color::SURFACE).alpha(0.12))
            .cursor_text()
            .track_focus(&focus)
            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                this.focus_field(field, window, cx)
            }))
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                this.key(field, event, window, cx)
            }))
            .child(
                div()
                    .relative()
                    .w_full()
                    .px(px(theme::space::FIELD_X))
                    .py(px(theme::space::FIELD_Y))
                    .flex()
                    .items_center()
                    .text_size(px(theme::type_scale::FIELD))
                    .text_color(rgb(theme::color::FOREGROUND))
                    .when(empty, |el| {
                        el.child(
                            div()
                                .absolute()
                                .inset_0()
                                .px(px(theme::space::FIELD_X))
                                .py(px(theme::space::FIELD_Y))
                                .text_size(px(theme::type_scale::FIELD))
                                .text_color(rgb(theme::color::MUTED))
                                .child(placeholder.to_owned()),
                        )
                    })
                    .child(div().child(value))
                    .when(focused, |el| {
                        el.child(
                            div()
                                .w(px(theme::geometry::HAIRLINE))
                                .h(px(theme::type_scale::FIELD + 2.0))
                                .bg(rgb(theme::color::ACCENT))
                                .with_animation(
                                    format!("fin-caret-{field:?}"),
                                    Animation::new(Duration::from_millis(900)).repeat(),
                                    |el, t| el.opacity(if t < 0.5 { 1.0 } else { 0.0 }),
                                ),
                        )
                    }),
            )
    }

    fn date_button(
        &self,
        label: &'static str,
        date: NaiveDate,
        target: DateTarget,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let open = self.picker == Some(Picker::Date(target));
        div()
            .flex()
            .flex_col()
            .gap(px(theme::space::DOT_GAP))
            .child(
                div()
                    .text_size(px(theme::type_scale::MINI))
                    .text_color(rgb(theme::color::MUTED))
                    .child(label),
            )
            .child(
                div()
                    .id(format!("fin-date-{target:?}"))
                    .w_full()
                    .px(px(theme::space::FIELD_X))
                    .py(px(theme::space::FIELD_Y))
                    .border_1()
                    .border_color(rgb(if open {
                        theme::color::ACCENT
                    } else {
                        theme::color::SURFACE
                    }))
                    .bg(rgb(theme::color::SURFACE).alpha(0.12))
                    .text_size(px(theme::type_scale::FIELD))
                    .text_color(rgb(theme::color::FOREGROUND))
                    .cursor_pointer()
                    .hover(|style| style.text_color(rgb(theme::color::ACCENT)))
                    .on_click(cx.listener(move |this, event: &ClickEvent, _, cx| {
                        this.toggle_date_picker_at(target, event.position(), cx)
                    }))
                    .child(format_date(date)),
            )
    }

    fn time_button(&self, target: TimeTarget, cx: &Context<Self>) -> impl IntoElement {
        let open = self.picker == Some(Picker::Time(target));
        div()
            .flex()
            .flex_col()
            .gap(px(theme::space::DOT_GAP))
            .child(
                div()
                    .text_size(px(theme::type_scale::MINI))
                    .text_color(rgb(theme::color::MUTED))
                    .child("time"),
            )
            .child(
                div()
                    .id(format!("fin-time-{target:?}"))
                    .w(px(theme::geometry::FIELD_TIME_W))
                    .px(px(theme::space::FIELD_X))
                    .py(px(theme::space::FIELD_Y))
                    .border_1()
                    .border_color(rgb(if open {
                        theme::color::ACCENT
                    } else {
                        theme::color::SURFACE
                    }))
                    .bg(rgb(theme::color::SURFACE).alpha(0.12))
                    .text_size(px(theme::type_scale::FIELD))
                    .text_color(rgb(theme::color::FOREGROUND))
                    .cursor_pointer()
                    .hover(|style| style.text_color(rgb(theme::color::ACCENT)))
                    .on_click(cx.listener(move |this, event: &ClickEvent, _, cx| {
                        this.toggle_time_picker(target, event.position(), cx)
                    }))
                    .child(format_time(self.time_for(target))),
            )
    }

    fn kind_button(&self, kind: Kind, cx: &Context<Self>) -> impl IntoElement {
        let selected = self.entry_kind == kind;
        let accent = match kind {
            Kind::Expense => theme::color::ACCENT,
            Kind::Income => theme::color::MINT,
        };
        div()
            .id(format!("fin-entry-kind-{}", kind.label()))
            .px(px(theme::space::ITEM_X))
            .py(px(theme::space::ITEM_Y - 2.))
            .border_1()
            .border_color(rgb(theme::color::SURFACE))
            .when(selected, |el| el.bg(rgb(theme::color::SURFACE)))
            .when(!selected, |el| el.bg(rgb(theme::color::SURFACE).alpha(0.08)))
            .text_size(px(theme::type_scale::NAV))
            .text_color(rgb(if selected {
                accent
            } else {
                theme::color::MUTED
            }))
            .cursor_pointer()
            .hover(|style| style.text_color(rgb(accent)))
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.entry_kind = kind;
                cx.notify();
            }))
            .child(kind.verb())
    }

    fn adjust_currency_button(
        &self,
        currency: Currency,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let selected = self.adjust_currency == currency;
        div()
            .id(format!("fin-adj-cur-{}", currency.label()))
            .px(px(theme::space::ITEM_X))
            .py(px(theme::space::ITEM_Y - 2.))
            .border_1()
            .border_color(rgb(theme::color::SURFACE))
            .when(selected, |el| el.bg(rgb(theme::color::SURFACE)))
            .when(!selected, |el| el.bg(rgb(theme::color::SURFACE).alpha(0.08)))
            .text_size(px(theme::type_scale::NAV))
            .text_color(rgb(if selected {
                theme::color::ACCENT
            } else {
                theme::color::MUTED
            }))
            .cursor_pointer()
            .hover(|style| style.text_color(rgb(theme::color::FOREGROUND)))
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.adjust_currency = currency;
                cx.notify();
            }))
            .child(currency.label())
    }

    fn category_button(&self, category: Category, cx: &Context<Self>) -> impl IntoElement {
        let selected = self.entry_category == category;
        div()
            .id(format!("fin-entry-cat-{}", category.label()))
            .px(px(theme::space::ITEM_X))
            .py(px(theme::space::ITEM_Y - 2.))
            .border_1()
            .border_color(rgb(theme::color::SURFACE))
            .when(selected, |el| el.bg(rgb(theme::color::SURFACE)))
            .when(!selected, |el| el.bg(rgb(theme::color::SURFACE).alpha(0.08)))
            .text_size(px(theme::type_scale::NAV))
            .text_color(rgb(if selected {
                theme::color::ACCENT
            } else {
                theme::color::MUTED
            }))
            .cursor_pointer()
            .hover(|style| style.text_color(rgb(theme::color::FOREGROUND)))
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.entry_category = category;
                cx.notify();
            }))
            .child(category.label())
    }

    fn currency_button(&self, currency: Currency, cx: &Context<Self>) -> impl IntoElement {
        let selected = self.entry_currency == currency;
        div()
            .id(format!("fin-cur-{}", currency.label()))
            .px(px(theme::space::ITEM_X))
            .py(px(theme::space::ITEM_Y - 2.))
            .border_1()
            .border_color(rgb(theme::color::SURFACE))
            .when(selected, |el| el.bg(rgb(theme::color::SURFACE)))
            .when(!selected, |el| el.bg(rgb(theme::color::SURFACE).alpha(0.08)))
            .text_size(px(theme::type_scale::NAV))
            .text_color(rgb(if selected {
                theme::color::ACCENT
            } else {
                theme::color::MUTED
            }))
            .cursor_pointer()
            .hover(|style| style.text_color(rgb(theme::color::FOREGROUND)))
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.entry_currency = currency;
                cx.notify();
            }))
            .child(currency.label())
    }

    fn chart_tab(&self, category: Category, cx: &Context<Self>) -> impl IntoElement {
        let selected = self.selected_category == category;
        div()
            .id(format!("fin-chart-cat-{}", category.label()))
            .px(px(theme::space::ITEM_X))
            .py(px(theme::space::ITEM_Y - 2.))
            .border_1()
            .border_color(rgb(theme::color::SURFACE))
            .when(selected, |el| el.bg(rgb(theme::color::SURFACE)))
            .when(!selected, |el| el.bg(rgb(theme::color::SURFACE).alpha(0.08)))
            .text_size(px(theme::type_scale::NAV))
            .text_color(rgb(if selected {
                theme::color::ACCENT
            } else {
                theme::color::MUTED
            }))
            .cursor_pointer()
            .hover(|style| style.text_color(rgb(theme::color::FOREGROUND)))
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.selected_category = category;
                this.slide = 0;
                cx.notify();
            }))
            .child(category.label())
    }

    fn arrow_button(
        label: &'static str,
        enabled: bool,
        id: &'static str,
        cx: &Context<Self>,
        on_click: impl Fn(&mut Self, &mut Context<Self>) + 'static,
    ) -> impl IntoElement {
        let mut builder = div()
            .id(id)
            .px(px(theme::space::DOT_GAP))
            .text_size(px(theme::type_scale::NAV))
            .text_color(rgb(if enabled {
                theme::color::FOREGROUND
            } else {
                theme::color::MUTED
            }));
        if enabled {
            builder = builder
                .cursor_pointer()
                .hover(|style| style.text_color(rgb(theme::color::ACCENT)))
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| on_click(this, cx)));
        }
        builder.child(label)
    }

    fn legend_item(color: u32, label: &'static str) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .gap(px(theme::space::DOT_GAP))
            .child(
                div()
                    .w(px(theme::geometry::LEGEND_W))
                    .h(px(theme::geometry::RULE_H))
                    .bg(rgb(color)),
            )
            .child(
                div()
                    .text_size(px(theme::type_scale::MINI))
                    .text_color(rgb(theme::color::MUTED))
                    .child(label),
            )
    }

    fn calendar(&self, target: DateTarget, cx: &Context<Self>) -> impl IntoElement {
        let month = self.cal_month;
        let selected = self.date_for(target);
        let today = Local::now().date_naive();

        let leading = month.weekday().num_days_from_monday() as usize;
        let mut cells: Vec<Option<u32>> = vec![None; leading];
        cells.extend((1..=days_in_month(month)).map(Some));
        while cells.len() % 7 != 0 {
            cells.push(None);
        }
        let rows: Vec<Vec<Option<u32>>> = cells.chunks(7).map(<[Option<u32>]>::to_vec).collect();

        let header = div()
            .flex()
            .items_center()
            .justify_between()
            .child(
                div()
                    .id(format!("fin-cal-prev-{target:?}"))
                    .px(px(theme::space::DOT_GAP))
                    .text_size(px(theme::type_scale::NAV))
                    .text_color(rgb(theme::color::MUTED))
                    .cursor_pointer()
                    .hover(|style| style.text_color(rgb(theme::color::ACCENT)))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.shift_calendar(-1, cx)
                    }))
                    .child("‹"),
            )
            .child(
                div()
                    .text_size(px(theme::type_scale::NAV))
                    .text_color(rgb(theme::color::FOREGROUND))
                    .child(month.format("%B %Y").to_string()),
            )
            .child(
                div()
                    .id(format!("fin-cal-next-{target:?}"))
                    .px(px(theme::space::DOT_GAP))
                    .text_size(px(theme::type_scale::NAV))
                    .text_color(rgb(theme::color::MUTED))
                    .cursor_pointer()
                    .hover(|style| style.text_color(rgb(theme::color::ACCENT)))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.shift_calendar(1, cx)
                    }))
                    .child("›"),
            );

        let weekdays = div().flex().children(["mo", "tu", "we", "th", "fr", "sa", "su"].map(
            |label| {
                div()
                    .w(px(theme::geometry::CAL_CELL))
                    .flex()
                    .justify_center()
                    .text_size(px(theme::type_scale::MINI))
                    .text_color(rgb(theme::color::MUTED))
                    .child(label)
            },
        ));

        let grid = rows.into_iter().map(move |row| {
            div().flex().children(row.into_iter().map(move |day| match day {
                None => div()
                    .w(px(theme::geometry::CAL_CELL))
                    .h(px(theme::geometry::CAL_ROW))
                    .into_any_element(),
                Some(day) => {
                    let date = NaiveDate::from_ymd_opt(month.year(), month.month(), day)
                        .expect("calendar day is valid");
                    let is_selected = date == selected;
                    let is_today = date == today;
                    div()
                        .id(format!("fin-cal-{target:?}-{day}"))
                        .w(px(theme::geometry::CAL_CELL))
                        .h(px(theme::geometry::CAL_ROW))
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_size(px(theme::type_scale::MINI))
                        .text_color(rgb(if is_selected {
                            theme::color::ACCENT
                        } else if is_today {
                            theme::color::FOREGROUND
                        } else {
                            theme::color::MUTED
                        }))
                        .when(is_selected, |el| el.bg(rgb(theme::color::SURFACE)))
                        .cursor_pointer()
                        .hover(|style| style.text_color(rgb(theme::color::ACCENT)))
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.choose_date(target, date, cx)
                        }))
                        .child(day.to_string())
                        .into_any_element()
                }
            }))
        });

        div()
            .flex()
            .flex_col()
            .gap(px(theme::space::TIGHT))
            .p(px(theme::space::TIGHT))
            .bg(rgb(theme::color::BACKGROUND))
            .occlude()
            .border_1()
            .border_color(rgb(theme::color::SURFACE))
            .w(px(theme::geometry::CAL_CELL * 7.0 + theme::space::TIGHT * 2.0))
            .max_w_full()
            .child(header)
            .child(weekdays)
            .children(grid)
    }

    fn time_panel(&self, target: TimeTarget, cx: &Context<Self>) -> impl IntoElement {
        let selected = self.time_for(target);
        let hour = selected.hour();
        let minute = selected.minute();

        let hour_grid = div()
            .flex()
            .flex_wrap()
            .w(px(theme::geometry::TIME_CELL * 6.0))
            .children((0..24).map(|h| {
                let selected = h == hour;
                div()
                    .id(format!("fin-hour-{h}"))
                    .w(px(theme::geometry::TIME_CELL))
                    .h(px(theme::geometry::CAL_ROW))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(theme::type_scale::MINI))
                    .text_color(rgb(if selected {
                        theme::color::ACCENT
                    } else {
                        theme::color::MUTED
                    }))
                    .when(selected, |el| el.bg(rgb(theme::color::SURFACE)))
                    .cursor_pointer()
                    .hover(|style| style.text_color(rgb(theme::color::ACCENT)))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        let time = NaiveTime::from_hms_opt(h, this.time_for(target).minute(), 0)
                            .expect("hour and minute are valid");
                        this.choose_time(target, time, cx)
                    }))
                    .child(format!("{h:02}"))
            }));

        let minute_grid = div()
            .flex()
            .flex_wrap()
            .w(px(theme::geometry::TIME_CELL * 6.0))
            .children((0..60).step_by(5).map(|m| {
                let selected = m == minute;
                div()
                    .id(format!("fin-minute-{m}"))
                    .w(px(theme::geometry::TIME_CELL))
                    .h(px(theme::geometry::CAL_ROW))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(theme::type_scale::MINI))
                    .text_color(rgb(if selected {
                        theme::color::ACCENT
                    } else {
                        theme::color::MUTED
                    }))
                    .when(selected, |el| el.bg(rgb(theme::color::SURFACE)))
                    .cursor_pointer()
                    .hover(|style| style.text_color(rgb(theme::color::ACCENT)))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        let time = NaiveTime::from_hms_opt(this.time_for(target).hour(), m, 0)
                            .expect("hour and minute are valid");
                        this.choose_time(target, time, cx)
                    }))
                    .child(format!("{m:02}"))
            }));

        div()
            .flex()
            .flex_col()
            .gap(px(theme::space::TIGHT))
            .p(px(theme::space::TIGHT))
            .bg(rgb(theme::color::BACKGROUND))
            .occlude()
            .border_1()
            .border_color(rgb(theme::color::SURFACE))
            .w(px(theme::geometry::TIME_CELL * 6.0 + theme::space::TIGHT * 2.0))
            .max_w_full()
            .child(Self::kicker("hour"))
            .child(hour_grid)
            .child(Self::kicker("minute"))
            .child(minute_grid)
            .child(
                div()
                    .id("fin-time-done")
                    .text_size(px(theme::type_scale::NAV))
                    .text_color(rgb(theme::color::ACCENT))
                    .cursor_pointer()
                    .hover(|style| style.text_color(rgb(theme::color::FOREGROUND)))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.picker = None;
                        cx.notify();
                    }))
                    .child("done"),
            )
    }

    fn picker_panel(&self, cx: &Context<Self>) -> AnyElement {
        match self.picker {
            Some(Picker::Date(target)) => self.calendar(target, cx).into_any_element(),
            Some(Picker::Time(target)) => self.time_panel(target, cx).into_any_element(),
            None => div().into_any_element(),
        }
    }

    /// Invisible full-window layer that closes the picker on any outside click.
    fn picker_backdrop(&self, window: &Window, cx: &Context<Self>) -> Option<AnyElement> {
        self.picker.map(|_| {
            let viewport = window.viewport_size();
            deferred(
                anchored().position(point(px(0.), px(0.))).child(
                    div()
                        .id("fin-picker-backdrop")
                        .w(px(viewport.width.as_f32()))
                        .h(px(viewport.height.as_f32()))
                        .occlude()
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, _: &gpui::MouseDownEvent, _, cx| {
                                this.picker = None;
                                cx.notify();
                            }),
                        ),
                ),
            )
            .with_priority(1)
            .into_any_element()
        })
    }

    /// Floating picker, clamped to the window so it can never open off-screen.
    fn picker_overlay(&self, cx: &Context<Self>) -> Option<AnyElement> {
        self.picker.map(|_| {
            deferred(
                anchored()
                    .snap_to_window_with_margin(px(theme::space::TIGHT))
                    .position(self.picker_anchor)
                    .offset(point(px(0.), px(theme::space::DOT_GAP)))
                    .child(self.picker_panel(cx)),
            )
            .with_priority(2)
            .into_any_element()
        })
    }

    fn slideshow(&self, summary: &Summary, cx: &Context<Self>) -> impl IntoElement {
        let statements = summary.statements();
        let index = if statements.is_empty() {
            0
        } else {
            self.slide % statements.len()
        };
        let dots = (0..statements.len()).map(|i| {
            div()
                .size(px(theme::geometry::DOT))
                .bg(rgb(if i == index {
                    theme::color::ACCENT
                } else {
                    theme::color::SURFACE
                }))
        });
        let current = statements.get(index).cloned().unwrap_or(stats::Statement {
            value: "—".into(),
            text: "no data yet".into(),
        });
        let count = statements.len();
        let can_navigate = count > 1;

        div()
            .id("fin-slideshow")
            .w_full()
            .flex()
            .flex_col()
            .gap(px(theme::space::TIGHT))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(theme::space::TIGHT))
                    .child(
                        div()
                            .flex()
                            .items_baseline()
                            .gap(px(theme::space::TIGHT))
                            .child(
                                div()
                                    .text_size(px(theme::type_scale::STATEMENT))
                                    .font_weight(FontWeight(theme::weight::STATEMENT))
                                    .text_color(rgb(theme::color::ACCENT))
                                    .child(current.value),
                            )
                            .child(
                                div()
                                    .text_size(px(theme::type_scale::NAV))
                                    .text_color(rgb(theme::color::FOREGROUND))
                                    .child(current.text),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(theme::space::TIGHT))
                            .child(Self::arrow_button(
                                "‹",
                                can_navigate,
                                "fin-slide-prev",
                                cx,
                                move |this, cx| {
                                    if count > 0 {
                                        this.slide = if this.slide == 0 {
                                            count - 1
                                        } else {
                                            this.slide - 1
                                        };
                                        cx.notify();
                                    }
                                },
                            ))
                            .child(div().flex().gap(px(theme::space::DOT_GAP)).children(dots))
                            .child(Self::arrow_button(
                                "›",
                                can_navigate,
                                "fin-slide-next",
                                cx,
                                move |this, cx| {
                                    if count > 0 {
                                        this.slide = (this.slide + 1) % count;
                                        cx.notify();
                                    }
                                },
                            )),
                    ),
            )
    }

    /// Bordered container shared by every finance block.
    fn card() -> gpui::Div {
        div()
            .flex()
            .flex_col()
            .gap(px(theme::space::TIGHT))
            .p(px(theme::space::CARD_PAD))
            .border_1()
            .border_color(rgb(theme::color::SURFACE))
    }

    fn metric(label: &'static str, value: String) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap(px(theme::space::DOT_GAP))
            .child(Self::hint(label))
            .child(
                div()
                    .text_size(px(theme::type_scale::NAV))
                    .text_color(rgb(theme::color::FOREGROUND))
                    .child(value),
            )
    }

    /// Top summary row: variance slideshow on the left, break-even and
    /// window metadata on the right.
    fn kpi_row(&self, summary: &Summary, cx: &Context<Self>) -> impl IntoElement {
        let daily = if summary.is_past || summary.available == 0 {
            "—".to_owned()
        } else {
            money(summary.threshold_round())
        };
        let plan = if summary.plan_active {
            money(summary.plan_now)
        } else {
            "—".to_owned()
        };

        div()
            .w_full()
            .flex()
            .items_stretch()
            .flex_shrink_0()
            .gap(px(theme::space::CARD_GAP))
            .child(
                Self::card()
                    .flex_1()
                    .min_w_0()
                    .child(self.slideshow(summary, cx)),
            )
            .child(
                Self::card()
                    .flex_1()
                    .min_w_0()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .items_baseline()
                            .gap(px(theme::space::TIGHT))
                            .child(
                                div()
                                    .text_size(px(theme::type_scale::STATEMENT))
                                    .font_weight(FontWeight(theme::weight::STATEMENT))
                                    .text_color(rgb(theme::color::ACCENT))
                                    .child(daily),
                            )
                            .child(Self::hint("break-even / day")),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .items_end()
                            .gap(px(theme::space::GROUP_Y))
                            .child(Self::metric(
                                "day",
                                format!("{} of {}", summary.day_index, summary.total_days),
                            ))
                            .child(Self::metric("left", money(summary.remaining)))
                            .child(Self::metric("average plan", plan)),
                    ),
            )
    }

    fn interpolate(points: &[(f32, i64)], day: f32) -> Option<i64> {
        let first = points.first()?;
        let last = points.last()?;
        if day < first.0 || day > last.0 {
            return None;
        }
        let mut value = first.1;
        for (x, money) in points {
            if *x <= day + 0.0001 {
                value = *money;
            } else {
                break;
            }
        }
        Some(value)
    }

    fn plan_at(summary: &Summary, day: f32) -> Option<i64> {
        if !summary.plan_available || day < summary.plan_start_x {
            return None;
        }
        let span = summary.total_days as f32 - summary.plan_start_x;
        if span <= f32::EPSILON {
            return Some(0);
        }
        Some(
            (summary.plan_amount as f64
                * ((summary.total_days as f32 - day) / span) as f64)
                .round() as i64,
        )
    }

    fn hover_overlay(&self, summary: &Summary, cx: &Context<Self>) -> Option<AnyElement> {
        let position = self.hover_position?;
        let bounds = self.chart_bounds.get();
        let width = bounds.size.width.as_f32();
        let height = bounds.size.height.as_f32();
        if width <= 0.0 || height <= 0.0 {
            return None;
        }

        let x = position.x.as_f32() - bounds.origin.x.as_f32();
        let y = position.y.as_f32() - bounds.origin.y.as_f32();
        if x < 0.0 || x > width || y < 0.0 || y > height {
            return None;
        }

        let pad = theme::space::CHART_PAD_X;
        let plot_width = (width - pad * 2.0).max(1.0);
        let fraction = ((x - pad) / plot_width).clamp(0.0, 1.0);
        let guide_x = pad + fraction * plot_width;
        let day = fraction * summary.total_days as f32;
        let seconds = (day as f64 * 86_400.0 / 60.0).round() as i64 * 60;
        let start_of_day = NaiveTime::from_hms_opt(0, 0, 0).expect("midnight is valid");
        let stamp = summary.start.and_time(start_of_day)
            + chrono::Duration::seconds(seconds);

        let actual = Self::interpolate(&summary.points, day);
        let plan = Self::plan_at(summary, day);
        let diff = actual.zip(plan).map(|(actual, plan)| actual - plan);
        let diff_color = match diff {
            Some(value) if value >= 0 => theme::color::MINT,
            Some(_) => theme::color::PURPLE,
            None => theme::color::MUTED,
        };
        let diff_text = match diff {
            Some(value) if value > 0 => format!("+{}", money(value)),
            Some(value) if value < 0 => format!("-{}", money(-value)),
            Some(_) => money(0),
            None => "—".to_owned(),
        };

        let tooltip_w = theme::geometry::HOVER_TOOLTIP_W;
        let left = (x + theme::geometry::HOVER_OFFSET)
            .min((width - tooltip_w).max(0.0));
        let top = (y - theme::geometry::HOVER_TOOLTIP_H - theme::geometry::HOVER_OFFSET)
            .max(0.0);

        let row = |label: &'static str, value: String, color: u32| {
            div()
                .flex()
                .justify_between()
                .gap(px(theme::space::TIGHT))
                .child(
                    div()
                        .text_size(px(theme::type_scale::MINI))
                        .text_color(rgb(theme::color::MUTED))
                        .child(label),
                )
                .child(
                    div()
                        .text_size(px(theme::type_scale::LEDGER))
                        .text_color(rgb(color))
                        .child(value),
                )
        };

        Some(
            div()
                .id("fin-chart-hover-overlay")
                .absolute()
                .inset_0()
                .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                    if !*hovered && this.hover_position.is_some() {
                        this.hover_position = None;
                        cx.notify();
                    }
                }))
                .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                    this.hover_position = Some(event.position);
                    cx.notify();
                }))
                .child(
                    div()
                        .absolute()
                        .top_0()
                        .left(px(guide_x))
                        .w(px(theme::geometry::HAIRLINE))
                        .h(px(height))
                        .bg(rgb(theme::color::SURFACE)),
                )
                .child(
                    div()
                        .absolute()
                        .left(px(left))
                        .top(px(top))
                        .w(px(tooltip_w))
                        .p(px(theme::space::TIGHT))
                        .bg(rgb(theme::color::BACKGROUND))
                        .border_1()
                        .border_color(rgb(theme::color::SURFACE))
                        .flex()
                        .flex_col()
                        .gap(px(theme::space::DOT_GAP))
                        .child(
                            div()
                                .text_size(px(theme::type_scale::MINI))
                                .text_color(rgb(theme::color::FOREGROUND))
                                .child(stamp.format("%d-%b-%Y %H:%M").to_string()),
                        )
                        .child(row(
                            "actual",
                            actual.map(money).unwrap_or_else(|| "—".to_owned()),
                            theme::color::ACCENT,
                        ))
                        .child(row(
                            "average plan",
                            plan.map(money).unwrap_or_else(|| "—".to_owned()),
                            theme::color::YELLOW,
                        ))
                        .child(row("difference", diff_text, diff_color)),
                )
                .into_any_element(),
        )
    }

    fn chart(&self, summary: &Summary, cx: &Context<Self>) -> impl IntoElement {
        let first = self.selected_period == 0;
        let last = self.selected_period + 1 == self.data.periods.len();
        let state = if summary.is_future {
            "future"
        } else if summary.is_past {
            "past"
        } else {
            "current"
        };

        Self::card()
            .w_full()
            .flex_grow(1.0)
            .min_h(px(theme::geometry::GRAPH_MIN_HEIGHT + 120.0))
            .child(
                div()
                    .w_full()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .justify_between()
                    .gap(px(theme::space::TIGHT))
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .items_center()
                            .gap(px(theme::space::TIGHT))
                            .child(Self::hint("viewing"))
                            .when(summary.combined, |el| {
                                el.child(
                                    div()
                                        .px(px(theme::space::ITEM_X))
                                        .py(px(theme::space::ITEM_Y - 2.))
                                        .border_1()
                                        .border_color(rgb(theme::color::SURFACE))
                                        .bg(rgb(theme::color::SURFACE))
                                        .text_size(px(theme::type_scale::NAV))
                                        .text_color(rgb(theme::color::MINT))
                                        .child("combined"),
                                )
                                .child(Self::hint("one balance · no category split"))
                            })
                            .when(!summary.combined, |el| {
                                el.child(self.chart_tab(Category::Nutrition, cx))
                                    .child(self.chart_tab(Category::Allowance, cx))
                            }),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(theme::space::TIGHT))
                            .child(Self::arrow_button(
                                "‹",
                                !first,
                                "fin-window-prev",
                                cx,
                                |this, cx| this.move_period(-1, cx),
                            ))
                            .child(
                                div()
                                    .text_size(px(theme::type_scale::MINI))
                                    .text_color(rgb(theme::color::MUTED))
                                    .child(format!(
                                        "window {} / {} · {state}{}",
                                        self.selected_period + 1,
                                        self.data.periods.len(),
                                        if summary.combined { " · combined" } else { "" }
                                    )),
                            )
                            .child(Self::arrow_button(
                                "›",
                                !last,
                                "fin-window-next",
                                cx,
                                |this, cx| this.move_period(1, cx),
                            )),
                    ),
            )
            .child(
                div()
                    .w_full()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .justify_between()
                    .gap(px(theme::space::TIGHT))
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .items_center()
                            .gap(px(theme::space::TIGHT))
                            .child(Self::kicker("money over the window"))
                            .child(Self::hint("hover for a minute-by-minute readout")),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .items_center()
                            .gap(px(theme::space::GROUP_Y))
                            .child(Self::legend_item(theme::color::ACCENT, "actual"))
                            .when(summary.plan_available, |el| {
                                el.child(Self::legend_item(
                                    theme::color::YELLOW,
                                    "average plan",
                                ))
                            })
                            .child(
                                div()
                                    .id("fin-toggle-combined")
                                    .text_size(px(theme::type_scale::MINI))
                                    .text_color(rgb(if summary.combined {
                                        theme::color::MINT
                                    } else {
                                        theme::color::MUTED
                                    }))
                                    .cursor_pointer()
                                    .hover(|style| {
                                        style.text_color(rgb(theme::color::ACCENT))
                                    })
                                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                        this.toggle_combined(cx)
                                    }))
                                    .child(if summary.combined {
                                        "combined · split into categories"
                                    } else {
                                        "combine this window into one balance"
                                    }),
                            ),
                    ),
            )
            .when(!summary.plan_available, |el| {
                el.child(
                    div()
                        .text_size(px(theme::type_scale::MINI))
                        .text_color(rgb(theme::color::YELLOW))
                        .child("average plan unavailable · the window's first transaction must be income"),
                )
            })
            .child(
                div()
                    .id("fin-chart-hover")
                    .relative()
                    .flex_grow(1.0)
                    .min_h(px(theme::geometry::GRAPH_MIN_HEIGHT))
                    .w_full()
                    .on_mouse_move(cx.listener(
                        |this, event: &MouseMoveEvent, _, cx| {
                            this.hover_position = Some(event.position);
                            cx.notify();
                        },
                    ))
                    .child(graph::plot(summary, self.chart_bounds.clone()))
                    .children(self.hover_overlay(summary, cx)),
            )
            .child(
                div()
                    .w_full()
                    .flex()
                    .justify_between()
                    .text_size(px(theme::type_scale::MINI))
                    .text_color(rgb(theme::color::MUTED))
                    .child(format!("last payday {}", format_date(summary.start)))
                    .child(format!(
                        "next payday {}",
                        format_date(summary.next_payday)
                    )),
            )
    }

    fn labeled(label: &'static str, child: impl IntoElement) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap(px(theme::space::DOT_GAP))
            .child(
                div()
                    .text_size(px(theme::type_scale::MINI))
                    .text_color(rgb(theme::color::MUTED))
                    .child(label),
            )
            .child(child)
    }

    fn hint(text: impl Into<gpui::SharedString>) -> impl IntoElement {
        div()
            .text_size(px(theme::type_scale::MINI))
            .text_color(rgb(theme::color::MUTED))
            .child(text.into())
    }

    fn entry(&self, cx: &Context<Self>, window: &Window) -> impl IntoElement {
        let enabled = parse_decimal(&self.amount).is_some_and(|amount| amount > 0.0);
        let amount_placeholder = format!("amount ({})", self.entry_currency.label());

        Self::card()
            .h_full()
            .child(Self::kicker("add transaction"))
            .child(
                div()
                    .w_full()
                    .flex()
                    .items_center()
                    .gap(px(theme::space::TIGHT))
                    .child(self.kind_button(Kind::Expense, cx))
                    .child(self.kind_button(Kind::Income, cx)),
            )
            .child(
                div()
                    .w_full()
                    .flex()
                    .items_center()
                    .gap(px(theme::space::TIGHT))
                    .child(self.currency_button(Currency::Czk, cx))
                    .child(self.currency_button(Currency::Eur, cx))
                    .child(self.currency_button(Currency::Azn, cx)),
            )
            .when(self.period().combined, |el| {
                el.child(Self::hint("combined window · category is ignored"))
            })
            .when(!self.period().combined, |el| {
                el.child(
                    div()
                        .w_full()
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .gap(px(theme::space::TIGHT))
                        .children(
                            Category::ALL
                                .into_iter()
                                .map(|category| self.category_button(category, cx)),
                        ),
                )
            })
            .child(self.field(Field::Amount, &amount_placeholder, window, cx))
            .child(self.field(Field::Note, "note (optional)", window, cx))
            .child(
                div()
                    .w_full()
                    .flex()
                    .items_end()
                    .gap(px(theme::space::TIGHT))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(self.date_button(
                                "date",
                                self.date,
                                DateTarget::Transaction,
                                cx,
                            )),
                    )
                    .child(self.time_button(TimeTarget::Transaction, cx))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(theme::space::TIGHT))
                            .child(Self::now_button(TimeTarget::Transaction, cx))
                            .child(Self::hint("enter adds")),
                    ),
            )
            .child(div().flex_grow(1.0))
            .child(
                div()
                    .id("fin-add")
                    .w_full()
                    .flex()
                    .justify_center()
                    .px(px(theme::space::ITEM_X))
                    .py(px(theme::space::ITEM_Y - 1.))
                    .border_1()
                    .border_color(rgb(theme::color::SURFACE))
                    .bg(rgb(theme::color::SURFACE).alpha(0.12))
                    .text_size(px(theme::type_scale::NAV))
                    .text_color(rgb(if enabled {
                        theme::color::ACCENT
                    } else {
                        theme::color::MUTED
                    }))
                    .cursor_pointer()
                    .hover(|style| style.text_color(rgb(theme::color::ACCENT)))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.add_transaction(cx)
                    }))
                    .child("add"),
            )
    }

    fn window_panel(&self, cx: &Context<Self>, window: &Window) -> impl IntoElement {
        let period = self.period();
        let combined = period.combined;
        let start = period.start;
        let end = period.end;

        Self::card()
            .h_full()
            .justify_between()
            .child(
                div()
                    .w_full()
                    .flex()
                    .flex_col()
                    .gap(px(theme::space::TIGHT))
                    .child(Self::kicker("window settings"))
                    .child(
                        div()
                            .w_full()
                            .flex()
                            .items_end()
                            .gap(px(theme::space::TIGHT))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .child(self.date_button(
                                        "last payday",
                                        start,
                                        DateTarget::PeriodStart,
                                        cx,
                                    )),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .child(self.date_button(
                                        "next payday",
                                        end,
                                        DateTarget::PeriodEnd,
                                        cx,
                                    )),
                            )
                            .when(combined, |el| {
                                el.child(
                                    div().flex_1().min_w_0().child(Self::labeled(
                                        "window balance",
                                        self.field(Field::CombinedBudget, "CZK", window, cx),
                                    )),
                                )
                            }),
                    )
                    .when(!combined, |el| {
                        el.child(
                            div()
                                .w_full()
                                .flex()
                                .items_end()
                                .gap(px(theme::space::TIGHT))
                                .child(
                                    div().flex_1().min_w_0().child(Self::labeled(
                                        "nutrition balance",
                                        self.field(Field::NutritionBudget, "CZK", window, cx),
                                    )),
                                )
                                .child(
                                    div().flex_1().min_w_0().child(Self::labeled(
                                        "allowance balance",
                                        self.field(Field::AllowanceBudget, "CZK", window, cx),
                                    )),
                                ),
                        )
                    }),
            )
            .child(
                div()
                    .w_full()
                    .flex()
                    .flex_col()
                    .gap(px(theme::space::TIGHT))
                    .child(
                        div()
                            .w_full()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .id("fin-add-window")
                                    .px(px(theme::space::ITEM_X))
                                    .py(px(theme::space::ITEM_Y - 2.))
                                    .border_1()
                                    .border_color(rgb(theme::color::SURFACE))
                                    .bg(rgb(theme::color::SURFACE).alpha(0.08))
                                    .text_size(px(theme::type_scale::MINI))
                                    .text_color(rgb(theme::color::MUTED))
                                    .cursor_pointer()
                                    .hover(|style| {
                                        style.text_color(rgb(theme::color::ACCENT))
                                    })
                                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                        this.open_new_window(cx)
                                    }))
                                    .child("+ add window"),
                            )
                            .child(Self::hint(if combined {
                                "one pot · enter to apply"
                            } else {
                                "enter to apply"
                            })),
                    )
                    .when_some(self.new_window, |el, (start, end)| {
                        el.child(
                            div()
                                .w_full()
                                .flex()
                                .flex_wrap()
                                .items_end()
                                .gap(px(theme::space::TIGHT))
                                .child(
                                    div().flex_1().min_w_0().child(self.date_button(
                                        "starts",
                                        start,
                                        DateTarget::NewStart,
                                        cx,
                                    )),
                                )
                                .child(
                                    div().flex_1().min_w_0().child(self.date_button(
                                        "ends",
                                        end,
                                        DateTarget::NewEnd,
                                        cx,
                                    )),
                                )
                                .child(
                                    div()
                                        .id("fin-create-window")
                                        .px(px(theme::space::ITEM_X))
                                        .py(px(theme::space::ITEM_Y - 2.))
                                        .border_1()
                                        .border_color(rgb(theme::color::SURFACE))
                                        .bg(rgb(theme::color::SURFACE).alpha(0.08))
                                        .text_size(px(theme::type_scale::MINI))
                                        .text_color(rgb(theme::color::ACCENT))
                                        .cursor_pointer()
                                        .hover(|style| {
                                            style.text_color(rgb(theme::color::FOREGROUND))
                                        })
                                        .on_click(cx.listener(
                                            |this, _: &ClickEvent, _, cx| {
                                                this.create_window(cx)
                                            },
                                        ))
                                        .child("add window"),
                                )
                                .child(
                                    div()
                                        .id("fin-cancel-window")
                                        .px(px(theme::space::ITEM_X))
                                        .py(px(theme::space::ITEM_Y - 2.))
                                        .border_1()
                                        .border_color(rgb(theme::color::SURFACE))
                                        .bg(rgb(theme::color::SURFACE).alpha(0.08))
                                        .text_size(px(theme::type_scale::MINI))
                                        .text_color(rgb(theme::color::MUTED))
                                        .cursor_pointer()
                                        .hover(|style| {
                                            style.text_color(rgb(theme::color::FOREGROUND))
                                        })
                                        .on_click(cx.listener(
                                            |this, _: &ClickEvent, _, cx| {
                                                this.new_window = None;
                                                cx.notify();
                                            },
                                        ))
                                        .child("cancel"),
                                ),
                        )
                    }),
            )
    }

    fn corrections(
        &self,
        summary: &Summary,
        window: &Window,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let period = self.period();
        let mut rows: Vec<(usize, &Adjustment)> = self
            .data
            .adjustments
            .iter()
            .enumerate()
            .filter(|(_, adj)| {
                (summary.combined || adj.category == self.selected_category)
                    && adj.date >= period.start
                    && adj.date < period.end
            })
            .collect();
        rows.sort_by(|a, b| {
            b.1.date
                .cmp(&a.1.date)
                .then_with(|| b.1.time.cmp(&a.1.time))
        });

        let has_rows = !rows.is_empty();
        let rows = rows.into_iter().map(|(index, adj)| {
            let signed = if adj.delta_czk > 0 {
                format!("+{}", money(adj.delta_czk))
            } else {
                format!("-{}", money(-adj.delta_czk))
            };
            div()
                .w_full()
                .min_w_0()
                .flex()
                .items_center()
                .gap(px(theme::space::TIGHT))
                .py(px(theme::space::LEDGER_ROW_Y))
                .border_b_1()
                .border_color(rgb(theme::color::SURFACE))
                .child(
                    div()
                        .flex_shrink_0()
                        .w(px(theme::geometry::LEDGER_DATE_W))
                        .text_size(px(theme::type_scale::MINI))
                        .text_color(rgb(theme::color::MUTED))
                        .child(format!(
                            "{} {}",
                            adj.date.format("%d-%b"),
                            adj.time.format("%H:%M")
                        )),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_size(px(theme::type_scale::LEDGER))
                        .text_color(rgb(theme::color::MUTED))
                        .child("balance correction"),
                )
                .child(
                    div()
                        .flex_shrink_0()
                        .text_size(px(theme::type_scale::LEDGER))
                        .text_color(rgb(theme::color::YELLOW))
                        .child(signed),
                )
                .child(
                    div()
                        .flex_shrink_0()
                        .id(format!("fin-del-adj-{index}"))
                        .px(px(theme::space::DOT_GAP))
                        .text_size(px(theme::type_scale::LEDGER))
                        .text_color(rgb(theme::color::MUTED))
                        .cursor_pointer()
                        .hover(|style| style.text_color(rgb(theme::color::PURPLE)))
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.delete_adjustment(index, cx)
                        }))
                        .child("×"),
                )
        });

        div()
            .flex()
            .flex_col()
            .gap(px(theme::space::TIGHT))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(Self::kicker("balance correction"))
                    .child(Self::hint("not a transaction")),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap(px(theme::space::TIGHT))
                    .child(
                        div()
                            .text_size(px(theme::type_scale::MINI))
                            .text_color(rgb(theme::color::MUTED))
                            .child("currency"),
                    )
                    .children(
                        Currency::ALL
                            .into_iter()
                            .map(|currency| self.adjust_currency_button(currency, cx)),
                    ),
            )
            .child(
                div()
                    .w_full()
                    .flex()
                    .items_end()
                    .gap(px(theme::space::TIGHT))
                    .child(
                        div().flex_1().min_w_0().child(self.date_button(
                            "at",
                            self.adjust_date,
                            DateTarget::Adjustment,
                            cx,
                        )),
                    )
                    .child(self.time_button(TimeTarget::Adjustment, cx))
                    .child(Self::now_button(TimeTarget::Adjustment, cx)),
            )
            .child(
                div()
                    .w_full()
                    .flex()
                    .items_end()
                    .gap(px(theme::space::TIGHT))
                    .child(
                        div().flex_1().min_w_0().child(Self::labeled(
                            "actual balance",
                            self.field(
                                Field::AdjustBalance,
                                self.adjust_currency.label(),
                                window,
                                cx,
                            ),
                        )),
                    )
                    .child(
                        div()
                            .id("fin-adjust")
                            .px(px(theme::space::ITEM_X))
                            .py(px(theme::space::ITEM_Y - 1.))
                            .bg(rgb(theme::color::SURFACE))
                            .text_size(px(theme::type_scale::NAV))
                            .text_color(rgb(theme::color::YELLOW))
                            .cursor_pointer()
                            .hover(|style| style.text_color(rgb(theme::color::FOREGROUND)))
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.apply_correction(cx)
                            }))
                            .child("correct"),
                    ),
            )
            .child(Self::hint(
                "sets the real balance at that moment · shifts this balance and everything after it",
            ))
            .when(has_rows, |el| {
                el.child(
                    div()
                        .id("fin-corrections")
                        .w_full()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .max_h(px(theme::space::LEDGER_MAX_HEIGHT))
                        .overflow_y_scroll()
                        .children(rows),
                )
            })
    }

    fn ledger(&self, summary: &Summary, cx: &Context<Self>) -> impl IntoElement {
        let period = self.period();
        let mut rows: Vec<(usize, &Transaction)> = self
            .data
            .transactions
            .iter()
            .enumerate()
            .filter(|(_, tx)| {
                (summary.combined || tx.category == self.selected_category)
                    && tx.date >= period.start
                    && tx.date < period.end
            })
            .collect();
        rows.sort_by(|a, b| {
            b.1.date
                .cmp(&a.1.date)
                .then_with(|| b.1.time.cmp(&a.1.time))
        });

        let empty = rows.is_empty();
        let rows = rows.into_iter().map(|(index, tx)| {
            let sign = match tx.kind {
                Kind::Expense => "-",
                Kind::Income => "+",
            };
            let amount = format!("{sign}{}", money(tx.amount_czk));
            let amount = if tx.currency == Currency::Czk {
                amount
            } else {
                format!(
                    "{amount} · {} {}",
                    trim_decimal(tx.original_amount),
                    tx.currency.label()
                )
            };
            let amount_color = match tx.kind {
                Kind::Expense => theme::color::FOREGROUND,
                Kind::Income => theme::color::MINT,
            };
            let note = if tx.note.trim().is_empty() {
                tx.category.label().to_owned()
            } else {
                tx.note.trim().to_owned()
            };
            div()
                .w_full()
                .min_w_0()
                .flex()
                .items_center()
                .gap(px(theme::space::TIGHT))
                .py(px(theme::space::LEDGER_ROW_Y))
                .border_b_1()
                .border_color(rgb(theme::color::SURFACE))
                .child(
                    div()
                        .flex_shrink_0()
                        .w(px(theme::geometry::LEDGER_DATE_W))
                        .text_size(px(theme::type_scale::MINI))
                        .text_color(rgb(theme::color::MUTED))
                        .child(format!(
                            "{} {}",
                            tx.date.format("%d-%b"),
                            tx.time.format("%H:%M")
                        )),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_size(px(theme::type_scale::LEDGER))
                        .text_color(rgb(theme::color::FOREGROUND))
                        .child(note),
                )
                .child(
                    div()
                        .flex_shrink_0()
                        .text_size(px(theme::type_scale::LEDGER))
                        .text_color(rgb(amount_color))
                        .child(amount),
                )
                .child(
                    div()
                        .flex_shrink_0()
                        .id(format!("fin-del-{index}"))
                        .px(px(theme::space::DOT_GAP))
                        .text_size(px(theme::type_scale::LEDGER))
                        .text_color(rgb(theme::color::MUTED))
                        .cursor_pointer()
                        .hover(|style| style.text_color(rgb(theme::color::PURPLE)))
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.delete_transaction(index, cx)
                        }))
                        .child("×"),
                )
        });

        div()
            .flex()
            .flex_col()
            .gap(px(theme::space::TIGHT))
            .child(Self::kicker(if summary.combined {
                "transactions · combined"
            } else {
                "transactions"
            }))
            .child(
                div()
                    .text_size(px(theme::type_scale::MINI))
                    .text_color(rgb(theme::color::MUTED))
                    .child(if summary.gained > 0 {
                        format!(
                            "{} · spent {} of {} · gained {}",
                            if summary.combined {
                                "combined"
                            } else {
                                self.selected_category.label()
                            },
                            money(summary.spent),
                            money(summary.available),
                            money(summary.gained)
                        )
                    } else {
                        format!(
                            "{} · {} of {}",
                            if summary.combined {
                                "combined"
                            } else {
                                self.selected_category.label()
                            },
                            money(summary.spent),
                            money(summary.available)
                        )
                    }),
            )
            .child(
                div()
                    .id("fin-ledger")
                    .w_full()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .max_h(px(theme::space::LEDGER_MAX_HEIGHT))
                    .overflow_y_scroll()
                    .when(empty, |el| {
                        el.child(
                            div()
                                .text_size(px(theme::type_scale::HINT))
                                .text_color(rgb(theme::color::MUTED))
                                .child("no transactions in this window yet"),
                        )
                    })
                    .children(rows),
            )
    }
    fn logs_card(
        &self,
        summary: &Summary,
        window: &Window,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        Self::card()
            .h_full()
            .child(
                div()
                    .w_full()
                    .flex()
                    .items_start()
                    .child(
                        div()
                            .flex_grow(1.2)
                            .flex_basis(px(0.))
                            .min_w_0()
                            .overflow_hidden()
                            .pr(px(theme::space::CARD_PAD))
                            .border_r_1()
                            .border_color(rgb(theme::color::SURFACE))
                            .child(self.ledger(summary, cx)),
                    )
                    .child(
                        div()
                            .flex_grow(1.0)
                            .flex_basis(px(0.))
                            .min_w_0()
                            .overflow_hidden()
                            .pl(px(theme::space::CARD_PAD))
                            .child(self.corrections(summary, window, cx)),
                    ),
            )
    }
}

impl Render for Finances {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let today = Local::now().date_naive();
        let summary = Summary::compute(
            &self.data,
            self.selected_period,
            self.selected_category,
            today,
        );

        // Flex items in this gpui version keep their content's minimum width, so
        // fractional columns feed back into the page width and squeeze the
        // sidebar. Pinning the page and its three bottom columns to the known
        // viewport geometry keeps the whole tab inside one screen.
        let viewport = window.viewport_size();
        let available = (viewport.width.as_f32()
            - theme::geometry::SIDEBAR_WIDTH
            - theme::space::CONTENT_X * 2.0)
            .max(600.0);
        let inner = available - theme::space::CARD_GAP * 2.0;
        let left_w = inner / 4.15;
        let mid_w = inner * 1.15 / 4.15;
        let right_w = inner * 2.0 / 4.15;
        // Subtract the app chrome above the finance content (brand, page
        // padding, breadcrumb/title/rule and their gaps) so the finance page
        // owns exactly the remaining viewport height.
        let available_h = (viewport.height.as_f32()
            - theme::geometry::BRAND_HEIGHT
            - theme::space::CONTENT_TOP * 2.0
            - theme::type_scale::KICKER
            - theme::type_scale::TITLE
            - theme::geometry::RULE_H
            - theme::space::KICKER_GAP * 3.0
            - theme::space::TIGHT
            - 4.0)
            .max(480.0);
        div()
            .id("finances-root")
            .w(px(available))
            .h(px(available_h))
            .flex()
            .flex_col()
            .gap(px(theme::space::CARD_GAP))
            .child(self.kpi_row(&summary, cx))
            .child(self.chart(&summary, cx))
            .child(
                div()
                    .w_full()
                    .flex()
                    .items_stretch()
                    .flex_shrink_0()
                    .gap(px(theme::space::CARD_GAP))
                    .child(
                        div()
                            .w(px(left_w))
                            .min_w_0()
                            .overflow_hidden()
                            .child(self.window_panel(cx, window)),
                    )
                    .child(
                        div()
                            .w(px(mid_w))
                            .min_w_0()
                            .overflow_hidden()
                            .child(self.entry(cx, window)),
                    )
                    .child(
                        div()
                            .w(px(right_w))
                            .min_w_0()
                            .overflow_hidden()
                            .child(self.logs_card(&summary, window, cx)),
                    ),
            )
            .when_some(self.status.clone(), |el, status| {
                el.child(
                    div()
                        .w_full()
                        .pt(px(theme::space::TIGHT))
                        .border_t_1()
                        .border_color(rgb(theme::color::SURFACE))
                        .text_size(px(theme::type_scale::HINT))
                        .text_color(rgb(theme::color::YELLOW))
                        .child(status),
                )
            })
            .children(self.picker_backdrop(window, cx))
            .children(self.picker_overlay(cx))
    }
}

fn amount_input(minor: i64) -> String {
    format!("{:.2}", minor as f64 / 100.0)
}

fn format_date(date: NaiveDate) -> String {
    date.format("%d-%b-%Y").to_string()
}

fn format_time(time: NaiveTime) -> String {
    time.format("%H:%M").to_string()
}
