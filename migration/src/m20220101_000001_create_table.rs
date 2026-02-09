use sea_orm_migration::prelude::extension::postgres::Type;
use sea_orm_migration::{prelude::*, schema::*};

#[derive(Iden)]
pub enum StatusEnum {
    Table,
    #[iden = "Pending"]
    Pending,
    #[iden = "Running"]
    Running,
    #[iden = "Success"]
    Success,
    #[iden = "Failure"]
    Failure,
}

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_type(
                Type::create()
                    .as_enum(StatusEnum::Table)
                    .values([
                        StatusEnum::Pending,
                        StatusEnum::Running,
                        StatusEnum::Success,
                        StatusEnum::Failure,
                    ])
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Job::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Job::Id)
                            .uuid()
                            .not_null()
                            .primary_key()
                            .default(Expr::cust("gen_random_uuid()")),
                    )
                    .col(string(Job::Url))
                    .col(string(Job::Method))
                    .col(json(Job::Headers))
                    .col(json(Job::Body))
                    .col(integer(Job::Retries))
                    .col(integer(Job::Attempts))
                    .col(
                        ColumnDef::new(Job::Status)
                            .enumeration(
                                StatusEnum::Table,
                                [
                                    StatusEnum::Pending,
                                    StatusEnum::Running,
                                    StatusEnum::Success,
                                    StatusEnum::Failure,
                                ],
                            )
                            .not_null()
                            .default(Expr::cust("'Pending'::status_enum")),
                    )
                    .col(timestamp(Job::NextRunAt))
                    .col(timestamp(Job::CreatedAt))
                    .col(timestamp(Job::UpdatedAt))
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Job::Table).to_owned())
            .await?;

        manager
            .drop_type(Type::drop().name(StatusEnum::Table).to_owned())
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum Job {
    Table,
    Id,
    Url,
    Method,
    Headers,
    Body,
    Retries,
    Attempts,
    Status,
    NextRunAt,
    CreatedAt,
    UpdatedAt,
}
