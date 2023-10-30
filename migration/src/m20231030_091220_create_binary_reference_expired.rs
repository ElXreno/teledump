use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                TableAlterStatement::new()
                    .table(Messages::Table)
                    .add_column(
                        ColumnDef::new(Messages::BinaryDataReferenceExpired)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                TableAlterStatement::new()
                    .table(Messages::Table)
                    .drop_column(Alias::new("binary_data_reference_expired"))
                    .to_owned(),
            )
            .await
    }
}

/// Learn more at https://docs.rs/sea-query#iden
#[derive(DeriveIden)]
enum Messages {
    Table,
    BinaryDataReferenceExpired,
}
