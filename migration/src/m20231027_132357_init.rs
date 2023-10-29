use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Messages::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Messages::Id).integer().not_null())
                    .col(ColumnDef::new(Messages::ChatId).big_integer().not_null())
                    .col(ColumnDef::new(Messages::UserId).big_integer().not_null())
                    .col(ColumnDef::new(Messages::Text).text().not_null())
                    .col(ColumnDef::new(Messages::HasBinaryData).boolean().not_null())
                    .col(ColumnDef::new(Messages::BinaryDataDownloaded).boolean().not_null())
                    .col(ColumnDef::new(Messages::BinaryDataPath).text())
                    .col(ColumnDef::new(Messages::BinaryDataType).text())
                    .col(ColumnDef::new(Messages::Date).date_time().not_null())
                    .primary_key(Index::create()
                        .name("pk-id_chat_id")
                        .col(Messages::Id)
                        .col(Messages::ChatId)
                        .primary())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Messages::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Messages {
    Table,
    Id,
    ChatId,
    UserId,
    Text,
    HasBinaryData,
    BinaryDataDownloaded,
    BinaryDataPath,
    BinaryDataType,
    Date,
}
