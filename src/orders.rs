//! Each promise belongs to one customer. Its position in the open list stays stable until dawn.
use serde::{Deserialize, Serialize};

use crate::day_cycle::Progress;
use crate::i18n::Lang;
use crate::inventory::Stock;
use crate::ui::{Action, PanelBuilder};

pub const PIN_MARK: &str = "★";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderState {
    #[default]
    Open,
    AtRegister,
    Collected,
    Expired,
}

/// Each ghost orders exactly one ware for the following night.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Order {
    pub ghost: String,
    pub product: String,
    pub night: u32,
    #[serde(default)]
    pub regular_name: Option<u32>,
    #[serde(default)]
    pub state: OrderState,
    #[serde(default)]
    pub pinned: bool,
}

impl Order {
    pub fn new(ghost: &str, product: &str, night: u32, regular_name: Option<u32>) -> Self {
        Self {
            ghost: ghost.into(),
            product: product.into(),
            night,
            regular_name,
            state: OrderState::Open,
            pinned: false,
        }
    }

    pub fn customer(&self, lang: &Lang) -> String {
        match self.regular_name {
            Some(name) => lang.t(&format!("regular.name.{name}")),
            None => lang.ghost(&self.ghost),
        }
    }

    pub fn ordered_ware(&self, lang: &Lang) -> String {
        lang.f(
            "count.entry",
            &[("item", &lang.product(&self.product)), ("count", &1)],
        )
    }

    pub fn label(&self, lang: &Lang) -> String {
        lang.f(
            "order_slips.line",
            &[
                ("who", &self.customer(lang)),
                ("item", &lang.product(&self.product)),
                ("count", &1),
                ("night", &self.night),
            ],
        )
    }

    fn status(&self, available: bool) -> &'static str {
        match self.state {
            OrderState::Open if available => "order_slips.ready",
            OrderState::Open => "order_slips.missing",
            OrderState::AtRegister => "order_slips.counter",
            OrderState::Collected => "order_slips.fulfilled",
            OrderState::Expired => "order_slips.expired",
        }
    }
}

impl Progress {
    pub fn pinned_for(&self, product: &str) -> bool {
        self.orders
            .iter()
            .any(|o| o.pinned && o.state == OrderState::Open && o.product == product)
    }

    pub fn set_order_state(&mut self, index: Option<usize>, state: OrderState) {
        if let Some(order) = index.and_then(|index| self.orders.get_mut(index)) {
            order.state = state;
        }
    }

    pub fn archive_due_orders(&mut self) {
        self.orders.retain_mut(|order| {
            if order.night > self.night {
                return true;
            }
            if order.state != OrderState::Collected {
                order.state = OrderState::Expired;
            }
            order.pinned = false;
            self.journal.order_history.push(order.clone());
            false
        });
    }
}

/// Allocate the visible stock once, in due order. An item on the counter has
/// already left the storeroom and does not claim another unit.
fn open_slips<'a>(
    orders: &'a [Order],
    stock: &'a Stock,
) -> impl Iterator<Item = (&'a Order, &'static str)> {
    let mut claimed = std::collections::BTreeMap::<&str, u32>::new();
    orders.iter().map(move |order| {
        let used = claimed.entry(&order.product).or_default();
        let available = *used < stock.count(&order.product);
        if order.state == OrderState::Open {
            *used += 1;
        }
        (order, order.status(available))
    })
}

pub fn book_page(
    ui: &mut PanelBuilder,
    lang: &Lang,
    progress: &Progress,
    stock: &Stock,
    page: usize,
) {
    ui.heading(&lang.t("order_slips.title"));
    ui.menu_option(ui.root(), &lang.t("menu.back"), Action::OpenBook, false);
    if progress.orders.is_empty() && progress.journal.order_history.is_empty() {
        ui.dim(&lang.t("order_slips.empty"));
    }
    for (idx, (order, status)) in open_slips(&progress.orders, stock).enumerate() {
        ui.rule();
        ui.text(&order.label(lang));
        ui.dim(&lang.t(status));
        if order.state == OrderState::Open {
            let key = if order.pinned {
                "order_slips.unpin"
            } else {
                "order_slips.pin"
            };
            ui.button_in(
                ui.root(),
                &lang.t(key),
                Action::PinOrder(idx),
                order.pinned,
                true,
            );
        }
    }
    const SLIPS_PER_PAGE: usize = 6;
    let history = &progress.journal.order_history;
    let pages = history.len().div_ceil(SLIPS_PER_PAGE);
    let page = page.min(pages.saturating_sub(1));
    if pages > 1 {
        let row = ui.row();
        if page > 0 {
            ui.button_in(
                row,
                &format!("← {}/{}", page, pages),
                Action::OpenOrders(page - 1),
                false,
                true,
            );
        }
        if page + 1 < pages {
            ui.button_in(
                row,
                &format!("{}/{} →", page + 2, pages),
                Action::OpenOrders(page + 1),
                false,
                true,
            );
        }
    }
    for order in history
        .iter()
        .rev()
        .skip(page * SLIPS_PER_PAGE)
        .take(SLIPS_PER_PAGE)
    {
        ui.rule();
        ui.text(&order.label(lang));
        ui.dim(&lang.t(order.status(false)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_unit_covers_only_one_slip_and_the_counter_does_not_claim_it_twice() {
        let mut progress = Progress::new(0);
        let order = Order::new("shy", "lavender", 2, None);
        progress.orders = vec![order.clone(), order];
        progress.orders[1].pinned = true;
        let mut stock = Stock::default();
        assert!(progress.pinned_for("lavender"));
        let status = |p: &Progress, s: &Stock| {
            open_slips(&p.orders, s)
                .map(|(_, key)| key)
                .collect::<Vec<_>>()
        };
        assert_eq!(
            status(&progress, &stock),
            ["order_slips.missing", "order_slips.missing"]
        );
        stock.add("lavender", 1);
        assert_eq!(
            status(&progress, &stock),
            ["order_slips.ready", "order_slips.missing"]
        );
        stock.take_one("lavender");
        progress.set_order_state(Some(1), OrderState::AtRegister);
        assert_eq!(
            status(&progress, &stock),
            ["order_slips.missing", "order_slips.counter"]
        );
        assert!(!progress.pinned_for("lavender"));
        stock.add("lavender", 1);
        assert_eq!(
            status(&progress, &stock),
            ["order_slips.ready", "order_slips.counter"]
        );
    }

    #[test]
    fn dawn_keeps_the_customer_and_due_night_on_both_outcomes() {
        let mut progress = Progress::new(0);
        progress.night = 4;
        let a = Order::new("cat", "fish_bone", 4, Some(2));
        let mut b = a.clone();
        b.pinned = true;
        b.state = OrderState::Collected;
        let tomorrow = Order::new("child", "marbles", 5, None);
        progress.orders = vec![a, b, tomorrow.clone()];
        progress.archive_due_orders();
        progress.archive_due_orders();
        assert_eq!(progress.orders, [tomorrow]);
        let history = &progress.journal.order_history;
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].state, OrderState::Expired);
        assert_eq!(history[1].state, OrderState::Collected);
        assert!(
            history
                .iter()
                .all(|o| o.regular_name == Some(2) && o.night == 4 && !o.pinned)
        );
        let copy: crate::book::Journal =
            ron::from_str(&ron::to_string(&progress.journal).unwrap()).unwrap();
        assert_eq!(copy.order_history, *history);
    }
}
