use crate::aneel_api::fetch_residential_tariffs;

#[tokio::main]
async fn main() {
    match fetch_residential_tariffs("ES").await {
        Ok(records) => println!("Fetched {} records for ES", records.len()),
        Err(e) => eprintln!("Error fetching: {}", e),
    }
}
