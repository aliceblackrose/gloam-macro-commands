use gloamwire::{
    http::{CreateInteractionResponseQuery, EditInteractionMessageQuery, EditWebhookMessage},
    model::{
        Embed, InteractionCallbackData, InteractionCallbackType, InteractionMessageData,
        InteractionResponse, Message, MessageFlags,
    },
};

use crate::{Context, Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ResponseState {
    Pending,
    Deferred { ephemeral: bool },
    Responded,
    Deleted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReplyAction {
    Initial { ephemeral: bool },
    CompleteDeferred,
    Followup { ephemeral: bool },
}

#[derive(Debug)]
enum MessagePayload {
    Content(String),
    Embeds(Vec<Embed>),
}

impl MessagePayload {
    fn into_message_data(self, ephemeral: bool) -> InteractionMessageData {
        let mut data = match self {
            Self::Content(content) => InteractionMessageData::content(content),
            Self::Embeds(embeds) => InteractionMessageData {
                embeds,
                ..Default::default()
            },
        };
        if ephemeral {
            data.flags = Some(MessageFlags::EPHEMERAL);
        }
        data
    }

    fn into_edit(self) -> EditWebhookMessage {
        match self {
            Self::Content(content) => EditWebhookMessage {
                content: Some(Some(content)),
                ..Default::default()
            },
            Self::Embeds(embeds) => EditWebhookMessage {
                embeds: Some(Some(embeds)),
                ..Default::default()
            },
        }
    }
}

impl ResponseState {
    fn reply_action(self, ephemeral: bool) -> Result<ReplyAction> {
        match self {
            Self::Pending => Ok(ReplyAction::Initial { ephemeral }),
            Self::Deferred {
                ephemeral: deferred_ephemeral,
            } => {
                if ephemeral != deferred_ephemeral {
                    return Err(Error::ResponseVisibilityMismatch);
                }
                Ok(ReplyAction::CompleteDeferred)
            }
            Self::Responded | Self::Deleted => Ok(ReplyAction::Followup { ephemeral }),
        }
    }

    const fn is_acknowledged(self) -> bool {
        !matches!(self, Self::Pending)
    }
}

impl<D> Context<D> {
    /// Sends a public interaction response.
    ///
    /// The first reply acknowledges a pending interaction. A reply after a
    /// public deferral edits the deferred original response, and later replies
    /// become followup messages automatically.
    pub async fn reply(&self, content: impl Into<String>) -> Result<()> {
        self.reply_inner(MessagePayload::Content(content.into()), false)
            .await
    }

    /// Sends a public interaction response containing one embed.
    pub async fn reply_embed(&self, embed: Embed) -> Result<()> {
        self.reply_embeds([embed]).await
    }

    /// Sends a public interaction response containing embeds.
    ///
    /// The first reply acknowledges a pending interaction. A reply after a
    /// public deferral edits the deferred original response, and later replies
    /// become followup messages automatically.
    pub async fn reply_embeds(&self, embeds: impl IntoIterator<Item = Embed>) -> Result<()> {
        self.reply_inner(MessagePayload::Embeds(embeds.into_iter().collect()), false)
            .await
    }

    /// Sends an ephemeral interaction response.
    ///
    /// The first reply acknowledges a pending interaction. A reply after an
    /// ephemeral deferral edits the deferred original response, and later
    /// replies become ephemeral followup messages. Deferral visibility cannot
    /// be changed while completing the original response.
    pub async fn reply_ephemeral(&self, content: impl Into<String>) -> Result<()> {
        self.reply_inner(MessagePayload::Content(content.into()), true)
            .await
    }

    /// Sends an ephemeral interaction response containing one embed.
    pub async fn reply_ephemeral_embed(&self, embed: Embed) -> Result<()> {
        self.reply_ephemeral_embeds([embed]).await
    }

    /// Sends an ephemeral interaction response containing embeds.
    ///
    /// The first reply acknowledges a pending interaction. A reply after an
    /// ephemeral deferral edits the deferred original response, and later
    /// replies become ephemeral followup messages. Deferral visibility cannot
    /// be changed while completing the original response.
    pub async fn reply_ephemeral_embeds(
        &self,
        embeds: impl IntoIterator<Item = Embed>,
    ) -> Result<()> {
        self.reply_inner(MessagePayload::Embeds(embeds.into_iter().collect()), true)
            .await
    }

    /// Defers the interaction with a public thinking response.
    pub async fn defer(&self) -> Result<()> {
        self.defer_inner(false).await
    }

    /// Defers the interaction with an ephemeral thinking response.
    pub async fn defer_ephemeral(&self) -> Result<()> {
        self.defer_inner(true).await
    }

    /// Edits the original interaction response.
    ///
    /// Completing a deferred response transitions it to the responded state.
    pub async fn edit_response(&self, content: impl Into<String>) -> Result<Message> {
        self.edit_response_inner(MessagePayload::Content(content.into()))
            .await
    }

    /// Replaces the embeds on the original interaction response with one embed.
    ///
    /// Completing a deferred response transitions it to the responded state.
    pub async fn edit_response_embed(&self, embed: Embed) -> Result<Message> {
        self.edit_response_embeds([embed]).await
    }

    /// Replaces the embeds on the original interaction response.
    ///
    /// Passing an empty iterator clears all embeds. Completing a deferred
    /// response transitions it to the responded state.
    pub async fn edit_response_embeds(
        &self,
        embeds: impl IntoIterator<Item = Embed>,
    ) -> Result<Message> {
        self.edit_response_inner(MessagePayload::Embeds(embeds.into_iter().collect()))
            .await
    }

    /// Deletes the original interaction response.
    pub async fn delete_response(&self) -> Result<()> {
        let mut state = self.response_state().lock().await;
        match *state {
            ResponseState::Pending => return Err(Error::InteractionNotAcknowledged),
            ResponseState::Deleted => return Err(Error::OriginalResponseDeleted),
            ResponseState::Deferred { .. } | ResponseState::Responded => {}
        }

        let interaction = self.interaction();
        self.rest()
            .delete_original_interaction_response(interaction.application_id, &interaction.token)
            .await?;
        *state = ResponseState::Deleted;
        Ok(())
    }

    /// Creates a public interaction followup message.
    pub async fn followup(&self, content: impl Into<String>) -> Result<Message> {
        self.followup_inner(MessagePayload::Content(content.into()), false)
            .await
    }

    /// Creates a public interaction followup message containing one embed.
    pub async fn followup_embed(&self, embed: Embed) -> Result<Message> {
        self.followup_embeds([embed]).await
    }

    /// Creates a public interaction followup message containing embeds.
    pub async fn followup_embeds(
        &self,
        embeds: impl IntoIterator<Item = Embed>,
    ) -> Result<Message> {
        self.followup_inner(MessagePayload::Embeds(embeds.into_iter().collect()), false)
            .await
    }

    /// Creates an ephemeral interaction followup message.
    pub async fn followup_ephemeral(&self, content: impl Into<String>) -> Result<Message> {
        self.followup_inner(MessagePayload::Content(content.into()), true)
            .await
    }

    /// Creates an ephemeral interaction followup message containing one embed.
    pub async fn followup_ephemeral_embed(&self, embed: Embed) -> Result<Message> {
        self.followup_ephemeral_embeds([embed]).await
    }

    /// Creates an ephemeral interaction followup message containing embeds.
    pub async fn followup_ephemeral_embeds(
        &self,
        embeds: impl IntoIterator<Item = Embed>,
    ) -> Result<Message> {
        self.followup_inner(MessagePayload::Embeds(embeds.into_iter().collect()), true)
            .await
    }

    async fn reply_inner(&self, payload: MessagePayload, ephemeral: bool) -> Result<()> {
        let mut state = self.response_state().lock().await;
        match state.reply_action(ephemeral)? {
            ReplyAction::Initial { ephemeral } => {
                let interaction = self.interaction();
                let response =
                    InteractionResponse::message(payload.into_message_data(ephemeral));
                self.rest()
                    .create_interaction_response(
                        interaction.id,
                        &interaction.token,
                        &response,
                        &CreateInteractionResponseQuery::default(),
                    )
                    .await?;
                *state = ResponseState::Responded;
            }
            ReplyAction::CompleteDeferred => {
                let interaction = self.interaction();
                let edit = payload.into_edit();
                self.rest()
                    .edit_original_interaction_response(
                        interaction.application_id,
                        &interaction.token,
                        &edit,
                        &EditInteractionMessageQuery::default(),
                    )
                    .await?;
                *state = ResponseState::Responded;
            }
            ReplyAction::Followup { ephemeral } => {
                let interaction = self.interaction();
                self.rest()
                    .create_followup_message(
                        interaction.application_id,
                        &interaction.token,
                        &payload.into_message_data(ephemeral),
                    )
                    .await?;
            }
        }

        Ok(())
    }

    async fn defer_inner(&self, ephemeral: bool) -> Result<()> {
        let mut state = self.response_state().lock().await;
        if !matches!(*state, ResponseState::Pending) {
            return Err(Error::InteractionAlreadyAcknowledged);
        }

        let interaction = self.interaction();
        let response = if ephemeral {
            let data = InteractionMessageData {
                flags: Some(MessageFlags::EPHEMERAL),
                ..Default::default()
            };
            InteractionResponse {
                kind: InteractionCallbackType::DEFERRED_CHANNEL_MESSAGE_WITH_SOURCE,
                data: Some(InteractionCallbackData::Message(Box::new(data))),
            }
        } else {
            InteractionResponse::new(InteractionCallbackType::DEFERRED_CHANNEL_MESSAGE_WITH_SOURCE)
        };

        self.rest()
            .create_interaction_response(
                interaction.id,
                &interaction.token,
                &response,
                &CreateInteractionResponseQuery::default(),
            )
            .await?;
        *state = ResponseState::Deferred { ephemeral };
        Ok(())
    }

    async fn edit_response_inner(&self, payload: MessagePayload) -> Result<Message> {
        let mut state = self.response_state().lock().await;
        match *state {
            ResponseState::Pending => return Err(Error::InteractionNotAcknowledged),
            ResponseState::Deleted => return Err(Error::OriginalResponseDeleted),
            ResponseState::Deferred { .. } | ResponseState::Responded => {}
        }

        let interaction = self.interaction();
        let edit = payload.into_edit();
        let message = self
            .rest()
            .edit_original_interaction_response(
                interaction.application_id,
                &interaction.token,
                &edit,
                &EditInteractionMessageQuery::default(),
            )
            .await?;

        if matches!(*state, ResponseState::Deferred { .. }) {
            *state = ResponseState::Responded;
        }
        Ok(message)
    }

    async fn followup_inner(&self, payload: MessagePayload, ephemeral: bool) -> Result<Message> {
        let acknowledged = self.response_state().lock().await.is_acknowledged();
        if !acknowledged {
            return Err(Error::InteractionNotAcknowledged);
        }

        let interaction = self.interaction();
        Ok(self
            .rest()
            .create_followup_message(
                interaction.application_id,
                &interaction.token,
                &payload.into_message_data(ephemeral),
            )
            .await?)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use gloamwire::model::{Embed, MessageFlags};
    use tokio::sync::Mutex;

    use super::{MessagePayload, ReplyAction, ResponseState};
    use crate::Error;

    #[test]
    fn plans_reply_actions_from_acknowledgement_state() {
        assert_eq!(
            ResponseState::Pending.reply_action(false).expect("initial"),
            ReplyAction::Initial { ephemeral: false }
        );
        assert_eq!(
            ResponseState::Deferred { ephemeral: false }
                .reply_action(false)
                .expect("deferred"),
            ReplyAction::CompleteDeferred
        );
        assert_eq!(
            ResponseState::Deferred { ephemeral: true }
                .reply_action(true)
                .expect("ephemeral deferred"),
            ReplyAction::CompleteDeferred
        );
        assert_eq!(
            ResponseState::Responded
                .reply_action(true)
                .expect("followup"),
            ReplyAction::Followup { ephemeral: true }
        );
        assert_eq!(
            ResponseState::Deleted
                .reply_action(false)
                .expect("followup after delete"),
            ReplyAction::Followup { ephemeral: false }
        );
    }

    #[test]
    fn rejects_visibility_changes_after_deferral() {
        assert!(matches!(
            ResponseState::Deferred { ephemeral: false }.reply_action(true),
            Err(Error::ResponseVisibilityMismatch)
        ));
        assert!(matches!(
            ResponseState::Deferred { ephemeral: true }.reply_action(false),
            Err(Error::ResponseVisibilityMismatch)
        ));
    }

    #[test]
    fn ephemeral_message_data_sets_only_requested_visibility_flag() {
        let public = MessagePayload::Content("public".to_owned()).into_message_data(false);
        assert_eq!(public.flags, None);

        let ephemeral = MessagePayload::Content("private".to_owned()).into_message_data(true);
        assert_eq!(ephemeral.flags, Some(MessageFlags::EPHEMERAL));
    }

    #[test]
    fn embed_payload_preserves_embeds_and_visibility() {
        let embed = Embed {
            title: Some("Status".to_owned()),
            description: Some("Online".to_owned()),
            ..Default::default()
        };

        let public = MessagePayload::Embeds(vec![embed.clone()]).into_message_data(false);
        assert_eq!(public.embeds, [embed.clone()]);
        assert_eq!(public.flags, None);

        let ephemeral = MessagePayload::Embeds(vec![embed]).into_message_data(true);
        assert_eq!(ephemeral.embeds.len(), 1);
        assert_eq!(ephemeral.flags, Some(MessageFlags::EPHEMERAL));
    }

    #[test]
    fn embed_edit_replaces_embed_collection() {
        let embed = Embed {
            title: Some("Updated".to_owned()),
            ..Default::default()
        };
        let edit = MessagePayload::Embeds(vec![embed.clone()]).into_edit();

        assert_eq!(edit.content, None);
        assert_eq!(edit.embeds, Some(Some(vec![embed])));
    }

    #[tokio::test]
    async fn response_lock_serializes_concurrent_reply_planning() {
        let state = Arc::new(Mutex::new(ResponseState::Pending));
        let mut first = state.lock().await;
        assert_eq!(
            first.reply_action(false).expect("first reply"),
            ReplyAction::Initial { ephemeral: false }
        );

        let second_state = Arc::clone(&state);
        let second = tokio::spawn(async move {
            let state = second_state.lock().await;
            state.reply_action(false)
        });

        tokio::task::yield_now().await;
        assert!(!second.is_finished());

        *first = ResponseState::Responded;
        drop(first);

        assert_eq!(
            second
                .await
                .expect("reply planner task")
                .expect("second reply"),
            ReplyAction::Followup { ephemeral: false }
        );
    }
}
