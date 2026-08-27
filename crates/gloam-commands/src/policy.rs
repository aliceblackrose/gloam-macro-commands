use std::{
    collections::HashMap,
    future::Future,
    pin::Pin,
    sync::Arc,
    time::{Duration, Instant},
};

use gloamwire::model::{InteractionContextType, Permissions, UserId};
use tokio::sync::{Mutex, Semaphore};

use crate::{Context, Error, Result};

/// Boxed future returned by generated command-check adapters.
pub type CheckFuture = Pin<Box<dyn Future<Output = Result<bool>> + Send + 'static>>;

/// Erased custom check function stored in a command policy.
pub type CheckHandler<D> = fn(Context<D>) -> CheckFuture;

/// One generated custom command check.
#[derive(Clone, Copy)]
pub struct CheckDescriptor<D> {
    name: &'static str,
    handler: CheckHandler<D>,
}

impl<D> CheckDescriptor<D> {
    /// Creates a named custom check descriptor.
    #[must_use]
    pub const fn new(name: &'static str, handler: CheckHandler<D>) -> Self {
        Self { name, handler }
    }

    /// Returns the check name used in diagnostics.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        self.name
    }

    /// Returns the erased custom check handler.
    #[must_use]
    pub const fn handler(&self) -> CheckHandler<D> {
        self.handler
    }
}

/// Runtime execution policy attached to one slash-command leaf.
///
/// Policy evaluation is deliberately separate from Discord registration
/// permissions. Restricted contexts and permissions are checked against the
/// received interaction immediately before the user handler runs.
pub struct CommandPolicy<D> {
    checks: Vec<CheckDescriptor<D>>,
    contexts: Vec<InteractionContextType>,
    member_permissions: Option<Permissions>,
    bot_permissions: Option<Permissions>,
    cooldown: Option<Duration>,
    cooldowns: Arc<Mutex<HashMap<UserId, Instant>>>,
    max_concurrency: Option<usize>,
    command_slots: Option<Arc<Semaphore>>,
}

impl<D> CommandPolicy<D> {
    /// Creates an unrestricted command policy.
    #[must_use]
    pub fn new() -> Self {
        Self {
            checks: Vec::new(),
            contexts: Vec::new(),
            member_permissions: None,
            bot_permissions: None,
            cooldown: None,
            cooldowns: Arc::new(Mutex::new(HashMap::new())),
            max_concurrency: None,
            command_slots: None,
        }
    }

    /// Appends one custom check in deterministic declaration order.
    #[must_use]
    pub fn check(mut self, check: CheckDescriptor<D>) -> Self {
        self.checks.push(check);
        self
    }

    /// Restricts execution to the supplied Discord interaction contexts.
    ///
    /// An empty set means unrestricted execution.
    #[must_use]
    pub fn contexts(
        mut self,
        contexts: impl IntoIterator<Item = InteractionContextType>,
    ) -> Self {
        self.contexts = contexts.into_iter().collect();
        self
    }

    /// Restricts execution to Discord guild interaction contexts.
    #[must_use]
    pub fn guild_only(self) -> Self {
        self.contexts([InteractionContextType::GUILD])
    }

    /// Requires the invoking guild member to have all supplied permissions.
    #[must_use]
    pub const fn member_permissions(mut self, permissions: Permissions) -> Self {
        self.member_permissions = Some(permissions);
        self
    }

    /// Requires the application to have all supplied permissions in the interaction channel.
    #[must_use]
    pub const fn bot_permissions(mut self, permissions: Permissions) -> Self {
        self.bot_permissions = Some(permissions);
        self
    }

    /// Adds a per-invoking-user cooldown to this command.
    #[must_use]
    pub const fn cooldown(mut self, duration: Duration) -> Self {
        self.cooldown = Some(duration);
        self
    }

    /// Sets an additional per-command concurrency limit.
    ///
    /// The global framework limit still applies. A zero value is rejected when
    /// the command tree is registered.
    #[must_use]
    pub fn max_concurrency(mut self, limit: usize) -> Self {
        self.max_concurrency = Some(limit);
        self.command_slots = Some(Arc::new(Semaphore::new(limit)));
        self
    }

    /// Returns custom checks in declaration order.
    #[must_use]
    pub fn checks(&self) -> &[CheckDescriptor<D>] {
        &self.checks
    }

    /// Returns allowed interaction contexts. An empty slice means unrestricted.
    #[must_use]
    pub fn allowed_contexts(&self) -> &[InteractionContextType] {
        &self.contexts
    }

    /// Returns required invoking-member permissions.
    #[must_use]
    pub const fn required_member_permissions(&self) -> Option<Permissions> {
        self.member_permissions
    }

    /// Returns required application permissions.
    #[must_use]
    pub const fn required_bot_permissions(&self) -> Option<Permissions> {
        self.bot_permissions
    }

    /// Returns the configured per-user cooldown.
    #[must_use]
    pub const fn cooldown_duration(&self) -> Option<Duration> {
        self.cooldown
    }

    /// Returns the configured per-command concurrency limit.
    #[must_use]
    pub const fn max_concurrent_executions(&self) -> Option<usize> {
        self.max_concurrency
    }

    pub(crate) fn command_slots(&self) -> Option<&Arc<Semaphore>> {
        self.command_slots.as_ref()
    }

    pub(crate) async fn evaluate(&self, context: &Context<D>) -> Result<()>
    where
        D: Send + Sync + 'static,
    {
        let path = context.command_path().join(" ");
        let interaction = context.interaction();

        if !self.contexts.is_empty()
            && interaction
                .context
                .is_none_or(|current| !self.contexts.contains(&current))
        {
            return Err(Error::CommandContextNotAllowed(path));
        }

        if let Some(required) = self.member_permissions {
            let actual = interaction
                .member
                .as_ref()
                .and_then(|member| member.permissions)
                .unwrap_or_else(Permissions::empty);
            if !actual.contains(required) {
                return Err(Error::MissingMemberPermissions {
                    path,
                    required,
                    actual,
                });
            }
        }

        if let Some(required) = self.bot_permissions {
            let actual = interaction.app_permissions.unwrap_or_else(Permissions::empty);
            if !actual.contains(required) {
                return Err(Error::MissingBotPermissions {
                    path,
                    required,
                    actual,
                });
            }
        }

        for check in &self.checks {
            if !(check.handler)(context.clone()).await? {
                return Err(Error::CommandCheckFailed {
                    path,
                    check: check.name,
                });
            }
        }

        if let Some(duration) = self.cooldown {
            self.reserve_cooldown(context, duration, path).await?;
        }

        Ok(())
    }

    async fn reserve_cooldown(
        &self,
        context: &Context<D>,
        duration: Duration,
        path: String,
    ) -> Result<()> {
        let interaction = context.interaction();
        let user_id = interaction
            .user
            .as_ref()
            .map(|user| user.id)
            .or_else(|| {
                interaction
                    .member
                    .as_ref()
                    .and_then(|member| member.user.as_ref())
                    .map(|user| user.id)
            })
            .ok_or_else(|| Error::CommandUserUnavailable(path.clone()))?;

        let now = Instant::now();
        let ready_at = now
            .checked_add(duration)
            .ok_or_else(|| Error::InvalidCommandPolicy(path.clone()))?;
        let mut cooldowns = self.cooldowns.lock().await;
        cooldowns.retain(|_, deadline| *deadline > now);

        if let Some(deadline) = cooldowns.get(&user_id)
            && *deadline > now
        {
            return Err(Error::CommandOnCooldown {
                path,
                retry_after: deadline.saturating_duration_since(now),
            });
        }

        cooldowns.insert(user_id, ready_at);
        Ok(())
    }
}

impl<D> Default for CommandPolicy<D> {
    fn default() -> Self {
        Self::new()
    }
}
