use rust_decimal::Decimal;
use serde::Deserialize;
use std::collections::HashMap;
use std::str::FromStr;

#[derive(Debug, Deserialize)]
struct NasaParameter {
    #[serde(rename = "ALLSKY_SFC_SW_DWN")]
    allsky_sw_dwn: Option<HashMap<String, f64>>,
}

#[derive(Debug, Deserialize)]
struct NasaProperties {
    parameter: NasaParameter,
}

#[derive(Debug, Deserialize)]
struct NasaResponse {
    properties: NasaProperties,
}

fn get_coordenates_by_state(state: &str) -> (f64, f64) {
    match state {
        "AC" => (-8.77, -70.55),
        "AL" => (-9.57, -36.78),
        "AM" => (-3.07, -61.66),
        "AP" => (1.41, -51.77),
        "BA" => (-12.55, -41.70),
        "CE" => (-5.20, -39.53),
        "DF" => (-15.78, -47.93),
        "ES" => (-19.18, -40.30),
        "GO" => (-15.82, -49.83),
        "MA" => (-5.42, -45.44),
        "MG" => (-18.51, -44.55),
        "MS" => (-20.77, -54.78),
        "MT" => (-12.68, -56.92),
        "PA" => (-1.99, -52.23),
        "PB" => (-7.24, -36.78),
        "PE" => (-8.81, -36.95),
        "PI" => (-7.71, -42.73),
        "PR" => (-24.89, -51.55),
        "RJ" => (-22.84, -43.15),
        "RN" => (-5.81, -36.59),
        "RO" => (-11.50, -63.58),
        "RR" => (2.73, -61.22),
        "RS" => (-30.03, -51.23),
        "SC" => (-27.24, -50.21),
        "SE" => (-10.57, -37.38),
        "SP" => (-22.19, -48.79),
        "TO" => (-10.18, -48.33),
        _ => (-15.78, -47.93), // DF as default
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn get_hsp_by_state(state: &str) -> Decimal {
    let (lat, lon) = get_coordenates_by_state(state);

    let url = format!(
        "https://power.larc.nasa.gov/api/temporal/climatology/point?parameters=ALLSKY_SFC_SW_DWN&community=RE&longitude={:.2}&latitude={:.2}&format=JSON",
        lon, lat
    );

    let client = reqwest::Client::new();

    let response = client.get(&url).send().await;

    if let Ok(res) = response {
        if let Ok(nasa_data) = res.json::<NasaResponse>().await {
            if let Some(mesure) = nasa_data.properties.parameter.allsky_sw_dwn {
                if let Some(&hsp_ann) = mesure.get("ANN") {
                    if hsp_ann > 0.0 {
                        if let Ok(hsp_decimal) = Decimal::from_str(&format!("{:.2}", hsp_ann)) {
                            return hsp_decimal;
                        }
                    }
                }
            }
        }
    }

    get_hsp_fallback(state)
}

#[cfg(target_arch = "wasm32")]
pub async fn get_hsp_by_state(state: &str) -> Decimal {
    use gloo_net::http::Request;

    let (lat, lon) = get_coordenates_by_state(state);

    let url = format!(
        "https://power.larc.nasa.gov/api/temporal/climatology/point?parameters=ALLSKY_SFC_SW_DWN&community=RE&longitude={:.2}&latitude={:.2}&format=JSON",
        lon, lat
    );

    let resp = Request::get(&url).send().await.unwrap();
    let nasa_data: NasaResponse = resp.json().await.unwrap();

    if let Some(mesure) = nasa_data.properties.parameter.allsky_sw_dwn {
        if let Some(&hsp_ann) = mesure.get("ANN") {
            if hsp_ann > 0.0 {
                return Decimal::from_str(&format!("{:.2}", hsp_ann)).unwrap();
            }
        }
    }

    get_hsp_fallback(state)
}

fn get_hsp_fallback(state: &str) -> Decimal {
    match state {
        "AC" => Decimal::from_str("4.40").unwrap(),
        "AL" => Decimal::from_str("5.20").unwrap(),
        "AM" => Decimal::from_str("4.20").unwrap(),
        "AP" => Decimal::from_str("5.00").unwrap(),
        "BA" => Decimal::from_str("5.40").unwrap(),
        "CE" => Decimal::from_str("5.60").unwrap(),
        "DF" => Decimal::from_str("5.10").unwrap(),
        "ES" => Decimal::from_str("4.80").unwrap(),
        "GO" => Decimal::from_str("5.20").unwrap(),
        "MA" => Decimal::from_str("5.30").unwrap(),
        "MG" => Decimal::from_str("5.10").unwrap(),
        "MS" => Decimal::from_str("5.00").unwrap(),
        "MT" => Decimal::from_str("5.10").unwrap(),
        "PA" => Decimal::from_str("4.90").unwrap(),
        "PB" => Decimal::from_str("5.50").unwrap(),
        "PE" => Decimal::from_str("5.40").unwrap(),
        "PI" => Decimal::from_str("5.60").unwrap(),
        "PR" => Decimal::from_str("4.30").unwrap(),
        "RJ" => Decimal::from_str("4.70").unwrap(),
        "RN" => Decimal::from_str("5.70").unwrap(),
        "RO" => Decimal::from_str("4.50").unwrap(),
        "RR" => Decimal::from_str("5.10").unwrap(),
        "RS" => Decimal::from_str("4.20").unwrap(),
        "SC" => Decimal::from_str("4.10").unwrap(),
        "SE" => Decimal::from_str("5.20").unwrap(),
        "SP" => Decimal::from_str("4.60").unwrap(),
        "TO" => Decimal::from_str("5.20").unwrap(),
        _ => Decimal::from_str("4.50").unwrap(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_coordinate_mapping() {
        let (lat_sp, lon_sp) = get_coordenates_by_state("SP");
        assert_eq!((lat_sp, lon_sp), (-22.19, -48.79));

        let (lat_df, lon_df) = get_coordenates_by_state("DF");
        assert_eq!((lat_df, lon_df), (-15.78, -47.93));

        //test fallback
        let (lat_inv, lon_inv) = get_coordenates_by_state("INVALID_STATE");
        assert_eq!((lat_inv, lon_inv), (-15.78, -47.93));
    }

    #[test]
    fn test_fallback_hsp_values() {
        let hsp_sp = get_hsp_fallback("SP");
        assert_eq!(hsp_sp, Decimal::from_str("4.60").unwrap());

        let hsp_es = get_hsp_fallback("ES");
        assert_eq!(hsp_es, Decimal::from_str("4.80").unwrap());
    }

    #[tokio::test]
    async fn test_get_hsp_by_state_real_api() {
        let hsp = get_hsp_by_state("SP").await;

        assert!(hsp > Decimal::from_str("3.0").unwrap());
        assert!(hsp < Decimal::from_str("7.0").unwrap());
    }

    #[test]
    fn test_nasa_json_deserialization() {
        let raw_json = r#"{
                "properties": {
                    "parameter": {
                        "ALLSKY_SFC_SW_DWN": {
                            "JAN": 5.4,
                            "FEB": 5.2,
                            "ANN": 4.85
                        }
                    }
                }
            }"#;

        let parsed: Result<NasaResponse, _> = serde_json::from_str(raw_json);
        assert!(parsed.is_ok());

        let nasa_data = parsed.unwrap();
        let measures = nasa_data.properties.parameter.allsky_sw_dwn.unwrap();

        assert_eq!(measures.get("ANN"), Some(&4.85));
    }
}
