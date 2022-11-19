use thiserror::Error;

/// Actor trait primary errors.
#[derive(Debug, Error)]
pub enum Error {
  /// This happens when it is unable to create a new supervior at the moment.
  #[error("Unable to create supervisor")]
  InitializeSupervisorFailed,
  /// This happens when it is unable to create a new children group at the moment.
  #[error("Unable to create children group")]
  InitializeChildrenGroupFailed,
}
