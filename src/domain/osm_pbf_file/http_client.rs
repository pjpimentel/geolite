// the one http agent the slice uses, for the catalogue and for the download alike. a vertical slice
// owns its own i/o, and reaching geofabrik is this slice's i/o just as sqlite is every other one's.

use std::sync::OnceLock;

use ureq::Agent;
use ureq::tls::{TlsConfig, TlsProvider};

const USER_AGENT: &str = concat!("geolite/", env!("CARGO_PKG_VERSION"));

static AGENT: OnceLock<Agent> = OnceLock::new();

pub fn agent() -> &'static Agent {
  AGENT.get_or_init(|| {
    let tls = TlsConfig::builder()
      .provider(TlsProvider::NativeTls)
      .build();
    let config = Agent::config_builder()
      .tls_config(tls)
      .user_agent(USER_AGENT)
      .build();
    Agent::new_with_config(config)
  })
}
