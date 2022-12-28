use crate::error::Error;

use std::time::Duration;

use anyhow::Result;
use async_trait::async_trait;
use bastion::prelude::*;
#[cfg(feature = "scaling")]
use bastion::resizer::OptimalSizeExploringResizer;

/// A `trait` to be implemented for state struct that attached to the Actor.
///
/// # Example
///
/// ```rust
/// # use std::time::Duration;
/// #
/// use bastor::Actor;
/// use anyhow::Result;
/// # use async_trait::async_trait;
/// use bastion::prelude::*;
/// # use cqrs_es::{Aggregate, DomainEvent};
/// # use serde::{Deserialize, Serialize};
/// # use thiserror::Error;
/// #
/// # #[tokio::main]
/// # async fn main() -> Result<()> {
/// #   Bastion::start();
/// #
/// #   MyInbox::default().run()?;
/// #   
/// #   Bastion::stop();
/// #   Ok(())
/// # }
/// #
/// #
/// # enum AggrVersion {
/// #   V1,
/// # }
/// #
/// # impl Into<String> for AggrVersion {
/// #   fn into(self) -> String {
/// #     match self {
/// #       Self::V1 => "1.0".to_owned(),
/// #     }
/// #   }
/// # }
/// #
/// # #[derive(Debug)]
/// # enum MyInboxCommand {
/// #   Add(String),
/// #   Remove(usize),
/// # }
/// #
/// # #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
/// # enum MyInboxEvent {
/// #   Added(String),
/// #   Removed(usize),
/// # }
/// # impl DomainEvent for MyInboxEvent {
/// #   fn event_type(&self) -> String {
/// #     "my_inbox_aggr".to_owned()
/// #   }
/// #
/// #   fn event_version(&self) -> String {
/// #     AggrVersion::V1.into()
/// #   }
/// # }
/// #
/// # #[derive(Debug, Error)]
/// # enum MyInboxError {
/// #   #[error("Add message `{0}` to inbox failed")]
/// #   AddFailed(String),
/// #   #[error("Remove message failed due to: `{0}`")]
/// #   RemoveFailed(String),
/// # }
/// #
/// # #[derive(Debug, Default, Clone, Serialize, Deserialize)]
/// # struct MyInbox {
/// #   messages: Vec<String>,
/// # }
/// #
/// # #[async_trait]
/// # impl Aggregate for MyInbox {
/// #   type Command = MyInboxCommand;
/// #
/// #   type Event = MyInboxEvent;
/// #
/// #   type Error = MyInboxError;
/// #
/// #   type Services = ();
/// #
/// #   fn aggregate_type() -> String {
/// #     "my_inbox_aggr".to_owned()
/// #   }
/// #
/// #   async fn handle(
/// #     &self,
/// #     command: Self::Command,
/// #     _: &Self::Services,
/// #   ) -> Result<Vec<Self::Event>, Self::Error> {
/// #     let mut events = vec![];
/// #     match command {
/// #       Self::Command::Add(msg) => events.push(Self::Event::Added(msg)),
/// #       Self::Command::Remove(position) => {
/// #         if self.messages.get(position).is_some() {
/// #           events.push(Self::Event::Removed(position))
/// #         } else {
/// #           return Err(Self::Error::RemoveFailed(format!(
/// #             "No element at `{position}`"
/// #           )));
/// #         }
/// #       }
/// #     }
/// #     Ok(events)
/// #   }
/// #
/// #   fn apply(&mut self, event: Self::Event) {
/// #     match event {
/// #       Self::Event::Added(msg) => self.messages.push(msg),
/// #       Self::Event::Removed(postion) => _ = self.messages.remove(postion),
/// #     }
/// #     println!("Event applied. New state: {self:?}")
/// #   }
/// # }
/// #
/// #[async_trait]
/// impl Actor for MyInbox {
///   async fn with_exec(mut state: Self, ctx: BastionContext) -> Result<(), ()> {
///     loop {
///       let msg = ctx.recv().await?;
///       let result = MessageHandler::new(msg)
///         .on_tell(|command: MyInboxCommand, _| -> Result<()> {
///           println!("{command:?}");
///           let events = run!(state.handle(command, &()))?;
///           Distributor::named("my_inbox").tell_everyone(events)?;
///           Ok(())
///         })
///         .on_tell(|events: Vec<MyInboxEvent>, _| {
///           println!("{events:?}");
///           events
///             .into_iter()
///             .for_each(|event| state.apply(event.clone()));
///           Ok(())
///         })
///         .on_tell(|_: (), _| panic!())
///         .on_fallback(|_, _| Ok(()));
///       match result {
///         Ok(_) => {}
///         Err(e) => println!("{e:?}"),
///       }
///     }
///   }
///
///   fn with_distributor(&self) -> Option<Distributor> {
///     Some(Distributor::named("my_inbox"))
///   }
///
///   fn with_children_callbacks(&self) -> Option<Callbacks> {
///     Some(Callbacks::new().with_after_restart(|| println!("Restarted!")))
///   }
///
///   fn with_redundancy(&self) -> Option<usize> {
///     Some(5)
///   }
/// }
/// ```
#[async_trait]
pub trait Actor: Clone + Sync + Send + 'static {
  /// Run the actor with defined handler at [`Actor::with_exec`] with `Bastion::supervisor`.
  fn run(&self) -> Result<()> {
    let supervisor = Bastion::supervisor(|mut sp| {
      if let Some(callbacks) = self.with_supervisor_callbacks() {
        sp = sp.with_callbacks(callbacks);
      }

      if let Some(strategy) = self.with_strategy() {
        sp = sp.with_strategy(strategy);
      }

      if let Some(restart_strategy) = self.with_restart_strategy() {
        sp = sp.with_restart_strategy(restart_strategy);
      }
      sp
    })
    .map_err(|_| Error::InitializeSupervisorFailed)?;
    self.run_with_supervisor(supervisor)?;
    Ok(())
  }

  /// Run the actor with defined handler at [`Actor::with_exec`] with passing the [`supervisor`](bastion::supervisor::SupervisorRef) into.
  fn run_with_supervisor(&self, parent: SupervisorRef) -> Result<()> {
    let state = self.clone();
    parent
      .children(move |mut c| {
        if let Some(callbacks) = self.with_children_callbacks() {
          c = c.with_callbacks(callbacks);
        }

        if let Some(dispatcher) = self.with_dispatcher() {
          c = c.with_dispatcher(dispatcher);
        }

        if let Some(distributor) = self.with_distributor() {
          c = c.with_distributor(distributor);
        }

        if let Some(interval) = self.with_heartbeat_tick() {
          c = c.with_heartbeat_tick(interval);
        }

        if let Some(name) = self.with_name() {
          c = c.with_name(name);
        }

        if let Some(redundancy) = self.with_redundancy() {
          c = c.with_redundancy(redundancy);
        }

        #[cfg(feature = "scaling")]
        if let Some(resizer) = Self::with_resizer() {
          c = c.with_resizer(resizer);
        }

        c.with_exec(move |ctx| {
          let state = state.clone();
          async move { Actor::with_exec(state, ctx).await }
        })
      })
      .map_err(|_| Error::InitializeChildrenGroupFailed)?;
    Ok(())
  }

  /// Main executor that needs to be implemented.
  async fn with_exec(mut state: Self, ctx: BastionContext) -> Result<(), ()>;

  // ============================================================================ //

  /// Sets the callbacks that will get called at this supervisor's
  /// different lifecycle events.
  ///
  /// See [`Callbacks`]'s documentation for more information about the
  /// different callbacks available.
  fn with_supervisor_callbacks(&self) -> Option<Callbacks> {
    None
  }

  /// Sets the strategy the supervisor should use when one
  /// of its supervised children groups or supervisors dies
  /// (in the case of a children group, it could be because one
  /// of its elements panicked or returned an error).
  ///
  /// The default strategy is
  /// [`SupervisionStrategy::OneForOne`].
  ///
  /// # Arguments
  ///
  /// * `strategy` - The strategy to use:
  ///     - [`SupervisionStrategy::OneForOne`] would only restart
  ///         the supervised children groups or supervisors that
  ///         fault.
  ///     - [`SupervisionStrategy::OneForAll`] would restart all
  ///         the supervised children groups or supervisors (even
  ///         those which were stopped) when one of them faults,
  ///         respecting the order in which they were added.
  ///     - [`SupervisionStrategy::RestForOne`] would restart the
  ///         supervised children groups or supervisors that fault
  ///         along with all the other supervised children groups
  ///         or supervisors that were added after them (even the
  ///         stopped ones), respecting the order in which they
  ///         were added.
  fn with_strategy(&self) -> Option<SupervisionStrategy> {
    None
  }

  /// Sets the actor restart strategy the supervisor should use
  /// of its supervised children groups or supervisors dies to
  /// restore in the correct state.
  ///
  /// The default strategy is the [`ActorRestartStrategy::Immediate`] and
  /// unlimited amount of retries.
  fn with_restart_strategy(&self) -> Option<RestartStrategy> {
    None
  }

  // ============================================================================ //

  /// Sets the callbacks that will get called at this children group's
  /// different lifecycle events.
  ///
  /// See [`Callbacks`]'s documentation for more information about the
  /// different callbacks available.
  ///
  /// # Arguments
  ///
  /// * `callbacks` - The callbacks that will get called for this
  ///     children group.
  fn with_children_callbacks(&self) -> Option<Callbacks> {
    None
  }

  /// Appends each supervised element to the declared dispatcher.
  ///
  /// By default supervised elements aren't added to any of dispatcher.
  ///
  /// # Arguments
  ///
  /// * `dispatcher` - An instance of struct that implements the
  /// [`DispatcherHandler`] trait.
  fn with_dispatcher(&self) -> Option<Dispatcher> {
    None
  }

  /// Appends a distributor to the children.
  ///
  /// By default supervised elements aren't added to any distributor.
  ///
  /// # Arguments
  ///
  /// * `distributor` - An instance of struct that implements the
  /// [`RecipientHandler`] trait.
  fn with_distributor(&self) -> Option<Distributor> {
    None
  }

  /// Overrides the default time interval for heartbeat onto
  /// the user defined.
  ///
  ///
  /// # Arguments
  ///
  /// * `interval` - The value of the [`std::time::Duration`] type
  fn with_heartbeat_tick(&self) -> Option<Duration> {
    None
  }

  /// Sets the name of this children group.
  fn with_name(&self) -> Option<String> {
    None
  }

  /// Sets the number of elements this children group will
  /// contain. Each element will call the closure passed in
  /// [`Actor::with_exec`] and run the returned future until it stops,
  /// panics or another element in the group stops or panics.
  ///
  /// The default number of elements a children group contains is `1`.
  ///
  /// # Arguments
  ///
  /// * `redundancy` - The number of elements this group will contain.
  fn with_redundancy(&self) -> Option<usize> {
    None
  }

  #[cfg(feature = "scaling")]
  /// Sets a custom resizer for the Children.
  ///
  /// This method is available only with the `scaling` feature flag.
  ///
  /// # Arguments
  ///
  /// * `resizer` - An instance of the [`Resizer`] struct.
  fn with_resizer(&self) -> Option<OptimalSizeExploringResizer> {
    None
  }
}
