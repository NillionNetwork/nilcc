use crate::funder::EthAmount;
use opentelemetry::{
    global,
    metrics::{Counter, Gauge, Meter, UpDownCounter},
};
use std::sync::LazyLock;

static METRICS: LazyLock<Metrics> = LazyLock::new(|| {
    let meter = global::meter("heartbeat-funder");
    Metrics::new(&meter)
});

pub(crate) fn get() -> &'static Metrics {
    &METRICS
}

pub(crate) struct Metrics {
    pub(crate) wallet: WalletMetrics,
    pub(crate) agents: AgentMetrics,
    pub(crate) addresses: AddressMetrics,
    // A private guard to prevent this type from being constructed outside of this module.
    _private: (),
}

impl Metrics {
    fn new(meter: &Meter) -> Self {
        let wallet = WalletMetrics::new(meter);
        let agents = AgentMetrics::new(meter);
        let addresses = AddressMetrics::new(meter);
        Self { wallet, agents, addresses, _private: () }
    }
}

pub(crate) struct AddressMetrics {
    monitored: UpDownCounter<i64>,
}

impl AddressMetrics {
    fn new(meter: &Meter) -> Self {
        let monitored = meter
            .i64_up_down_counter("nilcc.funder.addresses.monitored")
            .with_description("Total number of addresses monitored")
            .build();
        Self { monitored }
    }

    pub(crate) fn inc_monitored(&self, amount: usize) {
        self.monitored.add(amount as i64, &[]);
    }

    pub(crate) fn dec_monitored(&self, amount: usize) {
        self.monitored.add(-(amount as i64), &[]);
    }
}

pub(crate) struct AgentMetrics {
    monitored: Counter<u64>,
}

impl AgentMetrics {
    fn new(meter: &Meter) -> Self {
        let monitored = meter
            .u64_counter("nilcc.funder.agents.monitored")
            .with_description("Total number of agents monitored")
            .build();
        Self { monitored }
    }

    pub(crate) fn inc_monitored(&self, amount: u64) {
        self.monitored.add(amount, &[]);
    }
}

pub(crate) struct WalletMetrics {
    pub(crate) eth: WalletEthMetrics,
}

impl WalletMetrics {
    fn new(meter: &Meter) -> Self {
        Self { eth: WalletEthMetrics::new(meter) }
    }
}

pub(crate) struct WalletEthMetrics {
    funds: Gauge<f64>,
    payments: Counter<u64>,
    sent: Counter<f64>,
}

impl WalletEthMetrics {
    fn new(meter: &Meter) -> Self {
        let funds = meter
            .f64_gauge("nilcc.funder.wallet.eth.total")
            .with_description("Total amount of ETH available in wallet")
            .with_unit("ETH")
            .build();
        let payments = meter
            .u64_counter("nilcc.funder.wallet.eth.payments")
            .with_description("Total number of payments made")
            .build();
        let sent = meter
            .f64_counter("nilcc.funder.wallet.eth.sent")
            .with_description("Total amount of ETH sent")
            .with_unit("ETH")
            .build();
        Self { funds, payments, sent }
    }

    pub(crate) fn set_funds(&self, amount: EthAmount) {
        self.funds.record(amount.into(), &[]);
    }

    pub(crate) fn inc_payments(&self, amount: u64) {
        self.payments.add(amount, &[]);
    }

    pub(crate) fn inc_sent(&self, amount: EthAmount) {
        self.sent.add(amount.into(), &[]);
    }
}
