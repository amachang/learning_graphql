use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(User::Table)
                    .col(
                        ColumnDef::new(User::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(User::Slug)
                            .string()
                            .null()
                            .unique_key(),
                    )
                    .col(ColumnDef::new(User::Name).string().null())
                    .col(ColumnDef::new(User::Comment).string().null())
                    .col(ColumnDef::new(User::RegisteredAt).date_time().not_null())
                    .to_owned(),
            )
            .await?;
        manager
            .create_table(
                Table::create()
                    .table(Passkey::Table)
                    .col(
                        ColumnDef::new(Passkey::UserId)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Passkey::Content).json().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_passkey_user_id")
                            .from(Passkey::Table, Passkey::UserId)
                            .to(User::Table, User::Id)
                            .on_delete(ForeignKeyAction::Restrict)
                            .on_update(ForeignKeyAction::Restrict),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_table(
                Table::create()
                    .table(Post::Table)
                    .col(
                        ColumnDef::new(Post::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(Post::UserId)
                            .uuid()
                            .not_null()
                    )
                    .col(
                        ColumnDef::new(Post::Slug)
                            .string()
                            .null()
                            .unique_key(),
                        )
                    .col(ColumnDef::new(Post::Title).string().not_null())
                    .col(ColumnDef::new(Post::Content).string().not_null())
                    .col(ColumnDef::new(Post::CreatedAt).date_time().not_null())
                    .col(ColumnDef::new(Post::UpdatedAt).date_time().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_post_user_id")
                            .from(Post::Table, Post::UserId)
                            .to(User::Table, User::Id)
                            .on_delete(ForeignKeyAction::Restrict)
                            .on_update(ForeignKeyAction::Restrict),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_post_id_created_at")
                    .table(Post::Table)
                    .col(Post::Id)
                    .col(Post::CreatedAt)
                    .to_owned(),
            ).await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_post_created_at")
                    .table(Post::Table)
                    .col(Post::CreatedAt)
                    .to_owned(),
            ).await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(Index::drop().name("idx_post_created_at").to_owned())
            .await?;
        manager
            .drop_index(Index::drop().name("idx_post_id_created_at").to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(User::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Passkey::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Post::Table).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(DeriveIden)]
enum Post {
    Table,
    Id,
    UserId,
    Slug,
    Title,
    Content,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum User {
    Table,
    Id,
    Slug,
    Name,
    Comment,
    RegisteredAt,
}

#[derive(DeriveIden)]
enum Passkey {
    Table,
    UserId,
    Content,
}

