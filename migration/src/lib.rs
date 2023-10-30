pub use sea_orm_migration::prelude::*;

mod m20231027_132357_init;
mod m20231030_091220_create_binary_reference_expired;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20231027_132357_init::Migration),
            Box::new(m20231030_091220_create_binary_reference_expired::Migration),
        ]
    }
}
