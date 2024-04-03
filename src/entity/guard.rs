use async_graphql::{Context, Guard, Result};
use uuid::Uuid;
use super::user;

pub struct OwnerGuard {
    user_id: Uuid,
}

impl OwnerGuard {
    pub fn new(user_id: Uuid) -> Self {
        Self { user_id }
    }
}

impl Guard for OwnerGuard {
    async fn check(&self, ctx: &Context<'_>) -> Result<()> {
        if self.user_id != ctx.data::<user::Model>()?.id {
            Err("Forbidden".into())
        } else {
            Ok(())
        }
    }
}

