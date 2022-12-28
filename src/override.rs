use std::time::Duration;

use anyhow::Result;
use bastion::{
  prelude::{Dispatcher, Distributor},
  supervisor::{RestartStrategy, SupervisionStrategy, SupervisorRef},
  Bastion, Callbacks,
};

use crate::{Actor, Error};

/// An overridden actor that is used to override [`Actor`]'s Bastion settings.
#[derive(Debug)]
pub struct Override {
  supervisor: SupervisorRef,
  // For supervisor
  supervisor_callbacks: Option<Callbacks>,
  restart_strategy: Option<RestartStrategy>,
  strategy: Option<SupervisionStrategy>,
  // For children
  children_callbacks: Option<Callbacks>,
  dispatcher: Option<Dispatcher>,
  distributor: Option<Distributor>,
  heartbeat_tick: Option<Duration>,
  name: Option<String>,
  redundancy: Option<usize>,
  #[cfg(feature = "scaling")]
  resizer: Option<OptimalSizeExploringResizer>,
}

impl Default for Override {
  /// Default actor builder
  fn default() -> Self {
    Self {
      supervisor: Bastion::supervisor(|sp| sp).unwrap(),
      supervisor_callbacks: Default::default(),
      restart_strategy: Default::default(),
      strategy: Default::default(),
      children_callbacks: Default::default(),
      dispatcher: Default::default(),
      distributor: Default::default(),
      heartbeat_tick: Default::default(),
      name: Default::default(),
      redundancy: Default::default(),
      #[cfg(feature = "scaling")]
      resizer: Default::default(),
    }
  }
}

impl Override {
  /// Run the actor with defined handler at [`Actor::with_exec`]
  /// with [`Bastion::supervisor`](bastion::supervisor::Supervisor).
  pub fn run<A: Actor>(self, inner: A) -> Result<()> {
    let supervisor = Bastion::supervisor(|mut sp| {
      if let Some(callbacks) = inner.with_supervisor_callbacks() {
        sp = sp.with_callbacks(callbacks);
      }

      if let Some(strategy) = inner.with_strategy() {
        sp = sp.with_strategy(strategy);
      }

      if let Some(restart_strategy) = inner.with_restart_strategy() {
        sp = sp.with_restart_strategy(restart_strategy);
      }

      if let Some(ref callbacks) = self.supervisor_callbacks {
        sp = sp.with_callbacks(callbacks.clone());
      }

      if let Some(ref strategy) = self.strategy {
        sp = sp.with_strategy(strategy.clone());
      }

      if let Some(ref restart_strategy) = self.restart_strategy {
        sp = sp.with_restart_strategy(restart_strategy.clone());
      }
      sp
    })
    .map_err(|_| Error::InitializeSupervisorFailed)?;
    self.run_with_supervisor(supervisor, inner)?;
    Ok(())
  }

  /// Run the actor with defined handler at [`Actor::with_exec`]
  /// with passing the [`supervisor`](bastion::supervisor::SupervisorRef) into.
  pub fn run_with_supervisor<A: Actor>(
    self,
    parent: SupervisorRef,
    inner: A,
  ) -> Result<()> {
    let state = inner.clone();
    parent
      .children(move |mut c| {
        if let Some(callbacks) = inner.with_children_callbacks() {
          c = c.with_callbacks(callbacks);
        }

        if let Some(dispatcher) = inner.with_dispatcher() {
          c = c.with_dispatcher(dispatcher);
        }

        if let Some(distributor) = inner.with_distributor() {
          c = c.with_distributor(distributor);
        }

        if let Some(interval) = inner.with_heartbeat_tick() {
          c = c.with_heartbeat_tick(interval);
        }

        if let Some(name) = inner.with_name() {
          c = c.with_name(name);
        }

        if let Some(redundancy) = inner.with_redundancy() {
          c = c.with_redundancy(redundancy);
        }

        #[cfg(feature = "scaling")]
        if let Some(resizer) = A::with_resizer() {
          c = c.with_resizer(resizer);
        }

        if let Some(callbacks) = self.children_callbacks {
          c = c.with_callbacks(callbacks);
        }

        if let Some(dispatcher) = self.dispatcher {
          c = c.with_dispatcher(dispatcher);
        }

        if let Some(distributor) = self.distributor {
          c = c.with_distributor(distributor);
        }

        if let Some(interval) = self.heartbeat_tick {
          c = c.with_heartbeat_tick(interval);
        }

        if let Some(name) = self.name {
          c = c.with_name(name);
        }

        if let Some(redundancy) = self.redundancy {
          c = c.with_redundancy(redundancy);
        }

        #[cfg(feature = "scaling")]
        if let Some(resizer) = self.resizer {
          c = c.with_resizer(resizer);
        }

        c.with_exec(move |ctx| {
          let state = state.clone();
          async move { A::with_exec(state, ctx).await }
        })
      })
      .map_err(|_| Error::InitializeChildrenGroupFailed)?;
    Ok(())
  }

  /// Init actor with specified supervisor
  pub fn with_parent(mut self, sp: SupervisorRef) -> Self {
    self.supervisor = sp;
    self
  }

  /// Callbacks for the supervisor
  pub fn with_supervisor_callbacks(mut self, callbacks: Callbacks) -> Self {
    self.supervisor_callbacks = Some(callbacks);
    self
  }
  /// Supervision strategy for the supervisor
  pub fn with_stategy(mut self, strategy: SupervisionStrategy) -> Self {
    self.strategy = Some(strategy);
    self
  }
  /// Restart strategy for the supervisor
  pub fn with_restart_strategy(
    mut self,
    restart_strategy: RestartStrategy,
  ) -> Self {
    self.restart_strategy = Some(restart_strategy);
    self
  }
  // For children

  /// Callbacks for the supervisor's children
  pub fn with_children_callbacks(mut self, callbacks: Callbacks) -> Self {
    self.children_callbacks = Some(callbacks);
    self
  }

  /// Dispatcher for children
  pub fn with_dispatcher(mut self, dispatcher: Dispatcher) -> Self {
    self.dispatcher = Some(dispatcher);
    self
  }

  /// Distributor for children
  pub fn with_distributor(mut self, distributor: Distributor) -> Self {
    self.distributor = Some(distributor);
    self
  }

  /// Heartbeat interval
  pub fn with_heartbeat_tick(mut self, interval: Duration) -> Self {
    self.heartbeat_tick = Some(interval);
    self
  }

  /// Children group name
  pub fn with_name(mut self, name: impl Into<String>) -> Self {
    self.name = Some(name.into());
    self
  }

  /// Number of child in children group
  pub fn with_redundancy(mut self, redundancy: usize) -> Self {
    self.redundancy = Some(redundancy);
    self
  }

  #[cfg(feature = "scaling")]
  /// Resizer (for scalability)
  pub fn with_resizer(mut self, resizer: OptimalSizeExploringResizer) -> Self {
    self.resizer = Some(resizer);
    self
  }
}
