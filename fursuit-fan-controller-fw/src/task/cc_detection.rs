use defmt::debug;
use embassy_stm32::{
    Peri,
    adc::{ADC_MAX, Adc, VREF_INT},
    peripherals::{ADC1, PA4, PA5},
};
use embassy_time::{Duration, Ticker, Timer};

use crate::task::{MAIN_TASK_MESSAGES, MainTaskMessage};

#[embassy_executor::task]
pub async fn detect_cc(
    mut pa4: Peri<'static, PA4>,
    mut pa5: Peri<'static, PA5>,
    adc1: Peri<'static, ADC1>,
) -> ! {
    // Creates a high-level API for working with the ADC.
    // This also performs the STM32's built-in calibration routine.
    let mut adc = Adc::new(adc1);

    // Vrefint is a special channel that connects a chip-internal voltage reference
    // to the ADC. This is used to measure the actual supply voltage (VDDA) of the chip.
    let mut vrefint = adc.enable_vref();

    // Wait for Vrefint to become stable...
    Timer::after(Duration::from_millis(50)).await;

    // According to the datasheet (STM32F103C8, section 5.3.4), we must
    // sample for at least 17.1µs to definitely get an accurate value
    adc.set_sample_time(adc.sample_time_for_us(18));
    let vrefint_data = adc.read(&mut vrefint).await;
    debug!("Vrefint_data: {}", vrefint_data);

    // This allows us to calculate the actual supply voltage (VDDA) of the chip
    let vdda = (VREF_INT * ADC_MAX) / (vrefint_data as u32);
    debug!("Calculated supply voltage: {} mV", vdda);

    // ADC readings are relative to the supply voltage (VDDA), which we just calculated,
    // so we can now use this to convert the ADC readings to millivolts:
    let convert_to_millivolts = |adc_sample: u16| {
        // This formula can be found in section 15.3.32 of reference manual RM0316
        vdda * (adc_sample as u32) / ADC_MAX
    };

    // We sample every 5ms and require 10 consecutive identical readings
    // before we consider the power level to have changed. This is to
    // debounce the CC line readings. This means a change will be detected
    // after 50ms, which is within the 60ms requirement of the USB Type C spec.
    let mut ticker = Ticker::every(Duration::from_millis(5));
    let mut last_sent_power_level = None;
    let mut candidate_power_level = None;
    let mut consecutive_readings = 0;
    loop {
        let cc1_mv = convert_to_millivolts(adc.read(&mut pa4).await);
        let cc2_mv = convert_to_millivolts(adc.read(&mut pa5).await);
        // trace!("Measured CC1 = {} mV, CC2 = {} mV", cc1_mv, cc2_mv);

        let current_power_level = calculate_power_level(cc1_mv, cc2_mv);

        if Some(current_power_level) == candidate_power_level {
            consecutive_readings += 1;
        } else {
            candidate_power_level = Some(current_power_level);
            consecutive_readings = 1;
        }

        if consecutive_readings >= 10 {
            if candidate_power_level != last_sent_power_level {
                let enable_lockout = current_power_level == SuppliedUsbPowerLevel::Insufficient;
                MAIN_TASK_MESSAGES
                    .send(MainTaskMessage::SetLoadLockedOut(enable_lockout))
                    .await;
                last_sent_power_level = candidate_power_level;
            }
        }

        ticker.next().await;
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
enum SuppliedUsbPowerLevel {
    Insufficient,
    Sufficient,
}

// Minimum voltage level that has to be present on a CC line for that line to be
// considered active.
const ACTIVE_CC_LINE_VOLTAGE_THRESHOLD_MV: u32 = 200;

fn calculate_power_level(cc1_mv: u32, cc2_mv: u32) -> SuppliedUsbPowerLevel {
    // Determine which CC line is active. The inactive line will be near 0V.
    // We use a small threshold (e.g., 200mV) to be sure it's not just noise.
    let active_cc_mv = if cc1_mv > ACTIVE_CC_LINE_VOLTAGE_THRESHOLD_MV
        && cc2_mv < ACTIVE_CC_LINE_VOLTAGE_THRESHOLD_MV
    {
        cc1_mv
    } else if cc2_mv > ACTIVE_CC_LINE_VOLTAGE_THRESHOLD_MV
        && cc1_mv < ACTIVE_CC_LINE_VOLTAGE_THRESHOLD_MV
    {
        cc2_mv
    } else {
        // Either both are low (disconnected) or both are high (an audio accessory or debug adapter is connected).
        // For simple power sinking, we treat these cases as disconnected.
        return SuppliedUsbPowerLevel::Insufficient;
    };

    // These voltage values come from the USB Type-C Spec Release 2.0, table 4-36.
    // https://www.usb.org/sites/default/files/USB%20Type-C%20Spec%20R2.0%20-%20August%202019.pdf
    if active_cc_mv >= 700 && active_cc_mv < 2040 {
        // 1.5A or 3A
        SuppliedUsbPowerLevel::Sufficient
    } else {
        // too low or invalid reading
        SuppliedUsbPowerLevel::Insufficient
    }
}
