use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

pub async fn fetch_residential_tariffs(
    state_filter: &str,
) -> Result<Vec<AneelTariffRecord>, String> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        fetch_residential_tariffs_native(state_filter).await
    }

    #[cfg(target_arch = "wasm32")]
    {
        fetch_residential_tariffs_wasm(state_filter).await
    }
}

#[cfg(not(target_arch = "wasm32"))]
async fn fetch_residential_tariffs_native(
    state_filter: &str,
) -> Result<Vec<AneelTariffRecord>, String> {
    let url = "https://dadosabertos.aneel.gov.br/api/3/action/datastore_search";
    let resource_id = "fcf2906c-7c32-4b9b-a637-054e7a5234f4";

    let client = reqwest::Client::new();

    let response = client
        .get(url)
        .query(&[("resource_id", resource_id), ("limit", "10000")])
        .send()
        .await
        .map_err(|e| format!("Network error connecting to ANEEL: {}", e))?;

    // Capture raw response body for debugging
    let resp_text = response
        .text()
        .await
        .map_err(|e| format!("Network error connecting to ANEEL: {}", e))?;
    // Attempt to parse into expected structure
    let api_data: AneelApiResponse = serde_json::from_str(&resp_text).map_err(|parse_err| {
        eprintln!("Failed to parse JSON response. Raw body:\n{}", &resp_text);
        format!("Failed to get response text: {}", parse_err)
    })?;

    if !api_data.success {
        return Err("ANEEL API flagged the internal query operation as failed".to_string());
    }

    let filtered_records: Vec<AneelTariffRecord> = api_data
        .result
        .records
        .into_iter()
        .filter(|record| record.subgroup == "B1" && record.modality == "Convencional")
        .map(|mut record| {
            record.fio_b_tariff = record.distribution_tariff;
            record
        })
        .take(5)
        .collect();

    Ok(filtered_records)
}

#[cfg(target_arch = "wasm32")]
async fn fetch_residential_tariffs_wasm(
    state_filter: &str,
) -> Result<Vec<AneelTariffRecord>, String> {
    use std::collections::HashMap;
    use wasm_bindgen_futures::JsFuture;
    use web_sys::window;

    // Use statically included JSON snapshot
    const ANEEL_TARIFFS_JSON: &str = include_str!("../../assets/aneel_tariffs.json");
    let all_states_matrix: HashMap<String, Vec<AneelTariffRecord>> =
        serde_json::from_str(ANELEL_TARIFFS_JSON)
            .map_err(|e| format!("Failed to parse embedded JSON payload {:?}", e))?;

    if let Some(records) = all_states_matrix.get(state_filter) {
        Ok(records.clone())
    } else {
        Err(format!(
            "State filter '{}' not found inside target snapshot.",
            state_filter
        ))
    }
}

#[derive(Debug, Clone)]
pub struct UnifiedStateTariff {
    pub average_energy_tariff: Decimal,
    pub average_distribution_tariff: Decimal,
    pub average_fio_b_tariff: Decimal,
}

pub async fn get_unified_state_tariff(state_filter: &str) -> Result<UnifiedStateTariff, String> {
    let records = fetch_residential_tariffs(state_filter).await?;

    if records.is_empty() {
        return Err(format!(
            "No tariff data records found returned by ANEEL for state '{}'",
            state_filter
        ));
    }

    let total_utilities = Decimal::from(records.len());

    let sum_te: Decimal = records.iter().map(|rec| rec.energy_tariff).sum();
    let sum_tusd: Decimal = records.iter().map(|rec| rec.distribution_tariff).sum();
    let sum_fio_b: Decimal = records.iter().map(|rec| rec.fio_b_tariff).sum();

    Ok(UnifiedStateTariff {
        average_energy_tariff: sum_te / total_utilities,
        average_distribution_tariff: sum_tusd / total_utilities,
        average_fio_b_tariff: sum_fio_b / total_utilities,
    })
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AneelTariffRecord {
    #[serde(rename = "SigAgente", default)]
    pub utility_name: String,

    #[serde(rename = "DscSubGrupo", default)]
    pub subgroup: String,

    #[serde(rename = "DscModalidadeTarifaria", default)]
    pub modality: String,

    #[serde(
        rename = "VlrTE",
        deserialize_with = "deserialize_decimal_flexible",
        serialize_with = "serialize_decimal_as_str"
    )]
    pub energy_tariff: Decimal,

    #[serde(
        rename = "VlrTUSD",
        deserialize_with = "deserialize_decimal_flexible",
        serialize_with = "serialize_decimal_as_str"
    )]
    pub distribution_tariff: Decimal,

    #[serde(
        rename = "fio_b_tariff",
        default,
        deserialize_with = "deserialize_decimal_flexible",
        serialize_with = "serialize_decimal_as_str"
    )]
    pub fio_b_tariff: Decimal,
}

// Cleans up commas from raw web data and seamlessly parses dots from the JSON snapshot
fn deserialize_decimal_flexible<'de, D>(deserializer: D) -> Result<Decimal, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    let normalized = s.replace(",", ".");
    Decimal::from_str(&normalized).map_err(serde::de::Error::custom)
}

// Explicitly dumps strings to your JSON snapshot artifact so serde_wasm_bindgen doesn't complain
fn serialize_decimal_as_str<S>(decimal: &Decimal, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_str(&decimal.to_string())
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Deserialize)]
pub struct CkanResult {
    pub records: Vec<AneelTariffRecord>,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Deserialize)]
pub struct AneelApiResponse {
    pub success: bool,
    pub result: CkanResult,
}
