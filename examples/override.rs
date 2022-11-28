#![allow(unused)]

use std::time::Duration;

use anyhow::Result;
use async_trait::async_trait;
use bastion::prelude::*;
use bastor::{Actor, Override};
use cqrs_es::{Aggregate, DomainEvent};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[tokio::main]
async fn main() -> Result<()> {
  Bastion::start();

  Override::default()
    .with_distributor(Distributor::named("override"))
    .run(MyInbox::default())?;
  sleep(2).await;

  Distributor::named("override")
    .tell_one(MyInboxCommand::Add("Hello".to_owned()))?;
  sleep(2).await;

  Distributor::named("override").tell_one(())?;

  Bastion::block_until_stopped();
  Ok(())
}

async fn sleep(duration: u64) {
  tokio::time::sleep(Duration::from_secs(duration)).await
}

// ====================================================================== //

enum AggrVersion {
  V1,
}

impl Into<String> for AggrVersion {
  fn into(self) -> String {
    match self {
      Self::V1 => "1.0".to_owned(),
    }
  }
}

#[derive(Debug)]
enum MyInboxCommand {
  Add(String),
  Remove(usize),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
enum MyInboxEvent {
  Added(String),
  Removed(usize),
}
impl DomainEvent for MyInboxEvent {
  fn event_type(&self) -> String {
    "my_inbox_aggr".to_owned()
  }

  fn event_version(&self) -> String {
    AggrVersion::V1.into()
  }
}

#[derive(Debug, Error)]
enum MyInboxError {
  #[error("Add message `{0}` to inbox failed")]
  AddFailed(String),
  #[error("Remove message failed due to: `{0}`")]
  RemoveFailed(String),
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
struct MyInbox {
  messages: Vec<String>,
}

#[async_trait]
impl Aggregate for MyInbox {
  type Command = MyInboxCommand;

  type Event = MyInboxEvent;

  type Error = MyInboxError;

  type Services = ();

  fn aggregate_type() -> String {
    "my_inbox_aggr".to_owned()
  }

  async fn handle(
    &self,
    command: Self::Command,
    _: &Self::Services,
  ) -> Result<Vec<Self::Event>, Self::Error> {
    let mut events = vec![];
    match command {
      Self::Command::Add(msg) => events.push(Self::Event::Added(msg)),
      Self::Command::Remove(position) => {
        if self.messages.get(position).is_some() {
          events.push(Self::Event::Removed(position))
        } else {
          return Err(Self::Error::RemoveFailed(format!(
            "No element at `{position}`"
          )));
        }
      }
    }
    Ok(events)
  }

  fn apply(&mut self, event: Self::Event) {
    match event {
      Self::Event::Added(msg) => self.messages.push(msg),
      Self::Event::Removed(postion) => _ = self.messages.remove(postion),
    }
    println!("Event applied. New state: {self:?}")
  }
}

#[async_trait]
impl Actor for MyInbox {
  async fn with_exec(mut state: Self, ctx: BastionContext) -> Result<(), ()> {
    println!("{state:?}");
    loop {
      let msg = ctx.recv().await?;
      let result = MessageHandler::new(msg)
        .on_tell(|command: MyInboxCommand, _| -> Result<()> {
          println!("{command:?}");
          let events = run!(state.handle(command, &()))?;
          Distributor::named("override").tell_everyone(events)?;
          Ok(())
        })
        .on_tell(|events: Vec<MyInboxEvent>, _| {
          println!("{events:?}");
          events
            .into_iter()
            .for_each(|event| state.apply(event.clone()));
          Ok(())
        })
        .on_tell(|_: (), _| panic!())
        .on_fallback(|_, _| Ok(()));
      match result {
        Ok(_) => {}
        Err(e) => println!("{e:?}"),
      }
    }
  }

  fn with_children_callbacks() -> Option<Callbacks> {
    Some(Callbacks::new().with_after_restart(|| println!("Restarted!")))
  }

  fn with_redundancy() -> Option<usize> {
    Some(5)
  }
}
