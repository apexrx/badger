use dotenvy::dotenv;

mod entity;

#[tokio::main]
async fn main() {
    dotenv().ok();

    // Connect to database URL
    let db_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let db = sea_orm::Database::connect(db_url).await.unwrap();

    println!("Database connection established");
}
