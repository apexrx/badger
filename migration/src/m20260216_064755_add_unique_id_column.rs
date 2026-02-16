use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("TRUNCATE TABLE job")
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Job::Table)
                    .add_column(ColumnDef::new(Job::UniqueId).string().not_null())
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx-job-unique-id")
                    .table(Job::Table)
                    .col(Job::UniqueId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .get_connection()
            .execute_unprepared(
                "CREATE OR REPLACE FUNCTION prevent_unique_id_change()
        RETURNS TRIGGER AS $$
        BEGIN
            IF OLD.unique_id <> NEW.unique_id THEN
                RAISE EXCEPTION 'Cannot change unique_id';
            END IF;
            RETURN NEW;
        END;
        $$ LANGUAGE plpgsql;",
            )
            .await?;

        manager
            .get_connection()
            .execute_unprepared(
                " CREATE TRIGGER prevent_job_unique_id_update
                BEFORE UPDATE ON job
                FOR EACH ROW
                EXECUTE FUNCTION prevent_unique_id_change();",
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("DROP TRIGGER IF EXISTS prevent_job_unique_id_update ON job;")
            .await?;

        manager
            .get_connection()
            .execute_unprepared("DROP FUNCTION IF EXISTS prevent_unique_id_change();")
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx-job-unique-id")
                    .table(Job::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Job::Table)
                    .drop_column(Job::UniqueId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum Job {
    Table,
    UniqueId,
}
