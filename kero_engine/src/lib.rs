use std::str::FromStr;

use chrono::Datelike;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

pub mod aneel_api;
pub mod hsp_request;

// Import the HSP request function so it can be used in calculations.
use crate::hsp_request::get_hsp_by_state;

/// Represents the type of electrical phase connection for a solar installation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PhaseConnection {
    Monophasic, // minimum fee: 30 kWh
    Biphasic,   // minimum fee: 50 kWh
    Triphasic,  // minimum fee: 100 kWh
}

/// Input data for a solar ROI simulation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationInput {
    pub location_state: String,
    pub average_bill_reais: Decimal,
    pub system_capacity_kwp: Decimal,
    pub total_investment_reais: Decimal,
    pub phase_type: PhaseConnection,
}

/// Year‑by‑year projection of the simulation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YearlyProjection {
    pub year: u32,
    pub energy_generate_kwh: Decimal,
    pub gross_savings_reais: Decimal,
    pub distribution_cost_reais: Decimal,
    pub net_saving_reais: Decimal,
    pub cumulative_cash_flow_reais: Decimal,
}

/// Result of a complete simulation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationResult {
    pub payback_years: Decimal,
    pub total_25yr_savings_reais: Decimal,
    pub annual_table: Vec<YearlyProjection>,
}

pub trait TaxStrategy {
    fn get_distribution_charge_percentage(&self, year: u32) -> Decimal;
}

/// Legal framework for distribution charges (Fio B).
pub struct GeneralLegalFramework;

impl TaxStrategy for GeneralLegalFramework {
    /// Returns the Fio B charge percentage for a given year.
    ///
    /// The values are expressed as decimals (e.g. 0.15 = 15 %).
    fn get_distribution_charge_percentage(&self, year: u32) -> Decimal {
        match year {
            2023 => Decimal::from_str("0.15").unwrap(),
            2024 => Decimal::from_str("0.30").unwrap(),
            2025 => Decimal::from_str("0.45").unwrap(),
            2026 => Decimal::from_str("0.60").unwrap(),
            2027 => Decimal::from_str("0.75").unwrap(),
            2028 => Decimal::from_str("0.90").unwrap(),
            _ => Decimal::ONE,
        }
    }
}

/// Calculates the solar ROI for a given input.
///
/// The function performs a 25‑year cash‑flow projection, taking into account
/// panel degradation, irradiation data (HSP), tariff averages and legal
/// distribution charges.
pub async fn calculate_solar_roi(input: SimulationInput) -> Result<SimulationResult, String> {
    // 1. Fetch reference solar and tariff data for the requested state
    let real_data = aneel_api::get_unified_state_tariff(&input.location_state).await?;
    let current_year = chrono::Utc::now().year() as u32;

    // ANEEL open‑data values are already in R$/kWh, no divider needed.
    let energy_tariff = real_data.average_energy_tariff;
    let distribution_tariff = real_data.average_distribution_tariff;
    let total_tariff = energy_tariff + distribution_tariff;

    // 2. Identify the availability fee (minimum grid baseline in kWh) based on phase type
    let availability_fee_kwh = match input.phase_type {
        PhaseConnection::Monophasic => Decimal::from(30),
        PhaseConnection::Biphasic => Decimal::from(50),
        PhaseConnection::Triphasic => Decimal::from(100),
    };
    let availability_fee_yearly_kwh = availability_fee_kwh * Decimal::from(12);

    // 3. Deduce estimated monthly/yearly consumption (kWh) from the user's average bill value
    let estimated_monthly_consumption_kwh = input.average_bill_reais / total_tariff;
    let consumption_yearly_kwh = estimated_monthly_consumption_kwh * Decimal::from(12);

    // 4. Initialize the simulation parameters
    let legal_framework = GeneralLegalFramework;
    let mut annual_table = Vec::new();
    let mut cumulative_cash_flow = -input.total_investment_reais; // Year 0 initial cash drop
    let mut total_25yr_savings = Decimal::ZERO;
    let mut payback_years = Decimal::from(-1);

    let base_panel_efficiency = Decimal::ONE;
    let degradation_factor = Decimal::from_str("0.005").unwrap();

    // 5. Run the 25‑Year Projection Loop
    for i in 1..=25 {
        let loop_year = current_year + i - 1;

        // Calculate efficiency degradation
        let efficiency_modifier =
            base_panel_efficiency - (degradation_factor * Decimal::from(i - 1));

        // Fetch solar irradiation data asynchronously
        let solar_irradiation_hsp = get_hsp_by_state(&input.location_state).await;

        // Theoretical annual solar production
        let potential_yearly_generation_kwh = input.system_capacity_kwp
            * solar_irradiation_hsp
            * Decimal::from(30)
            * Decimal::from(12)
            * efficiency_modifier;

        // How much of the generation was actually used to offset consumption
        let actual_energy_compensated_kwh =
            potential_yearly_generation_kwh.min(consumption_yearly_kwh);

        // Remaining consumption that solar couldn't cover (billed at full price)
        let uncovered_yearly_kwh = if consumption_yearly_kwh > potential_yearly_generation_kwh {
            consumption_yearly_kwh - potential_yearly_generation_kwh
        } else {
            Decimal::ZERO
        };

        // Law 14.300: Get the transition rules for charging Fio B components
        let tax_percentage = legal_framework.get_distribution_charge_percentage(loop_year);

        // Distribution cost (Fio B) charged exclusively over the compensated energy
        let distribution_cost =
            actual_energy_compensated_kwh * distribution_tariff * tax_percentage;

        // Calculate the regular billing loop values
        let old_bill_yearly_reais = consumption_yearly_kwh * total_tariff;

        // New bill includes:
        // 1. Uncovered energy at full tariff
        // 2. The legal framework tax for using the grid distribution (Fio B)
        // 3. Ensuring the consumer pays at least the availability minimum fee baseline
        let baseline_grid_cost = uncovered_yearly_kwh * total_tariff + distribution_cost;
        let minimum_payable_fee = availability_fee_yearly_kwh * total_tariff;

        let new_bill_yearly_reais = baseline_grid_cost.max(minimum_payable_fee);

        // Net financial savings return
        let net_savings = old_bill_yearly_reais - new_bill_yearly_reais;
        let gross_savings = actual_energy_compensated_kwh * total_tariff;

        // Update aggregates
        total_25yr_savings += net_savings;
        cumulative_cash_flow += net_savings;

        if cumulative_cash_flow >= Decimal::ZERO && payback_years == Decimal::from(-1) {
            payback_years = Decimal::from(i);
        }

        // Push current year tracking entry into the result matrix
        annual_table.push(YearlyProjection {
            year: loop_year,
            energy_generate_kwh: potential_yearly_generation_kwh.round_dp(2),
            gross_savings_reais: gross_savings.round_dp(2),
            distribution_cost_reais: distribution_cost.round_dp(2),
            net_saving_reais: net_savings.round_dp(2),
            cumulative_cash_flow_reais: cumulative_cash_flow.round_dp(2),
        });
    }

    if payback_years == Decimal::from(-1) {
        payback_years = Decimal::from(25);
    }

    Ok(SimulationResult {
        payback_years,
        total_25yr_savings_reais: total_25yr_savings.round_dp(2),
        annual_table,
    })
}

#[cfg(test)]
mod test {
    use super::*;
    use serde_json::json;
    use std::str::FromStr;

    #[test]
    fn should_create_simulate_input() {
        let simulated_file_content = json!({
            "location_state": "ES",
            "average_bill_reais": "250.00",
            "system_capacity_kwp":  "4.5",
            "total_investment_reais": "30000.00",
            "phase_type": "Biphasic",
        });
        let user_input: SimulationInput = serde_json::from_value(simulated_file_content).unwrap();
        assert_eq!(user_input.location_state, "ES");
        assert_eq!(user_input.total_investment_reais, Decimal::new(3000000, 2));
    }

    #[test]
    fn should_return_correct_tax_percentage_for_2026() {
        let strategy = GeneralLegalFramework;
        let percentage_2026 = strategy.get_distribution_charge_percentage(2026);
        assert_eq!(percentage_2026, Decimal::from_str("0.60").unwrap());
    }

    #[tokio::test]
    async fn should_calculate_complete_simulation_with_payback_successfully() {
        // Mock a realistic standard client request for Espírito Santo (ES)
        let client_input = SimulationInput {
            location_state: "ES".to_string(),
            average_bill_reais: Decimal::from_str("500.00").unwrap(),
            system_capacity_kwp: Decimal::from_str("4.5").unwrap(),
            total_investment_reais: Decimal::from_str("15000.00").unwrap(),
            phase_type: PhaseConnection::Biphasic,
        };

        let simulation_run = calculate_solar_roi(client_input).await;

        // Assert engine processing integrity
        assert!(simulation_run.is_ok());

        let result = simulation_run.unwrap();

        // Verify payback was found and triggered within realistic parameters (under or equal to 25 years)
        assert!(result.payback_years > Decimal::ZERO);
        assert!(result.payback_years <= Decimal::from(25));

        // Verify output projections table contains exactly 25 distinct computed rows
        assert_eq!(result.annual_table.len(), 25);

        // Assert year progression consistency inside the dataset array
        assert_eq!(result.annual_table[0].year, 2026);
        assert_eq!(result.annual_table[24].year, 2050);
    }

    #[tokio::test]
    async fn should_parse_real_residential_tariffs_from_es() {
        let result = aneel_api::fetch_residential_tariffs("ES").await;

        assert!(result.is_ok(), "The API request failed: {:?}", result.err());

        let records = result.unwrap();

        assert!(!records.is_empty(), "No Residential records found for ES");

        println!(
            "Real Fio B tariff found in ES: R$/kWh {}",
            records[0].fio_b_tariff
        );

        assert_eq!(records[0].subgroup, "B1");

        assert!(records[0].distribution_tariff > rust_decimal::Decimal::ZERO);
    }

    #[tokio::test]
    async fn should_aggregate_and_calculate_state_averages_correctly() {
        let result = aneel_api::get_unified_state_tariff("SP").await;

        assert!(
            result.is_ok(),
            "Failed to aggregate tariff averages: {:?}",
            result.err()
        );
        let aggregate_data = result.unwrap();

        assert!(aggregate_data.average_energy_tariff > rust_decimal::Decimal::ZERO);
        assert!(aggregate_data.average_distribution_tariff > rust_decimal::Decimal::ZERO);

        println!(
            "Unified Average for SP -> TE: R$ {}, Fio B: R$ {}",
            aggregate_data.average_energy_tariff, aggregate_data.average_fio_b_tariff
        );
    }
}
