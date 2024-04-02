use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Post::Table)
                    .col(
                        ColumnDef::new(Post::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(Post::AuthorId)
                            .integer()
                            .not_null()
                    )
                    .col(ColumnDef::new(Post::Title).string().not_null())
                    .col(ColumnDef::new(Post::Text).string().not_null())
                    .foreign_key(
                        ForeignKey::create()
                        .name("fk_author_id")
                        .from(Post::Table, Post::AuthorId)
                        .to(Author::Table, Author::Id)
                        .on_delete(ForeignKeyAction::Restrict)
                        .on_update(ForeignKeyAction::Restrict),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_table(
                Table::create()
                    .table(Author::Table)
                    .col(
                        ColumnDef::new(Author::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Author::Name).string().not_null())
                    .to_owned(),
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Post::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Author::Table).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(DeriveIden)]
enum Post {
    Table,
    Id,
    AuthorId,
    Title,
    Text,
}

#[derive(DeriveIden)]
enum Author {
    Table,
    Id,
    Name,
}

