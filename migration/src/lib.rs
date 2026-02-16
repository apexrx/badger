pub use sea_orm_migration::prelude::*;

mod m20220101_000001_create_table;
mod m20260215_070659_add_check_in_column;
mod m20260216_064755_add_unique_id_column;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20220101_000001_create_table::Migration),
            Box::new(m20260215_070659_add_check_in_column::Migration),
            Box::new(m20260216_064755_add_unique_id_column::Migration),
        ]
    }
}
