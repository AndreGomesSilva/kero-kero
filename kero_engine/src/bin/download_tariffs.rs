use kero_engine::aneel_api::fetch_residential_tariffs;
use std::collections::HashMap;
use std::fs::File;
use std::io::Write;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Stating ANEEL tariff snapshot download for all states...");

    let states = vec![
        "AC", "AL", "AP", "AM", "BA", "CE", "DF", "ES", "GO", "MA", "MT", "MS", "MG", "PA", "PB",
        "PR", "PE", "PI", "RJ", "RN", "RS", "RO", "RR", "SC", "SP", "SE", "TO",
    ];

    let mut tariff_map = HashMap::new();

    for state in states {
        println!("Fetching data for state: {}...", state);

        match fetch_residential_tariffs(state).await {
            Ok(records) => {
                if !records.is_empty() {
                    tariff_map.insert(state.to_string(), records);
                }
            }
            Err(e) => eprintln!("Failed to fetch tariffs for {}: {}", state, e),
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
    }
    let json_data = serde_json::to_string_pretty(&tariff_map)?;

    let path = "assets/aneel_tariffs.json";
    std::fs::create_dir_all("assets")?;

    let mut file = File::create(path)?;
    file.write_all(json_data.as_bytes())?;

    println!("Tariff snapshot matrix successfully written to: {}", path);
    Ok(())
}
