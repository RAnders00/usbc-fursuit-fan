#![no_main]
#![no_std]

use cortex_m_rt::entry;
use fursuit_fan_controller_fw::{self as _, task}; // global logger + panicking-behavior + memory layout

use defmt::info;
use embassy_executor::Executor;
use embassy_stm32::{
    Config, adc, bind_interrupts,
    peripherals::ADC1,
    rcc::{ADCPrescaler, APBPrescaler, Pll, PllMul, PllPreDiv, PllSource, Sysclk},
};
use static_cell::StaticCell;

static EXECUTOR: StaticCell<Executor> = StaticCell::new();

bind_interrupts!(struct Irqs {
    ADC1_2 => adc::InterruptHandler<ADC1>;
});

#[entry]
fn main() -> ! {
    let mut config = Config::default();
    {
        // HSI = 8 MHz
        config.rcc.hsi = true;
        config.rcc.hse = None;

        // 8 MHz / 2 (fixed) * 16 = 64 MHz
        config.rcc.pll = Some(Pll {
            src: PllSource::HSI,
            prediv: PllPreDiv::DIV2,
            mul: PllMul::MUL16,
        });

        // SYSCLK is the 64 MHz of the PLL
        config.rcc.sys = Sysclk::PLL1_P;

        // APB1 runs at 64 MHz (max would be 72 MHz),
        // APB2 at 32 MHz (max would be 32 MHz)
        config.rcc.apb1_pre = APBPrescaler::DIV2;
        config.rcc.apb2_pre = APBPrescaler::DIV1;

        // Makes the ADCs run at 8 MHz
        // (Divides the APB2 clock)
        config.rcc.adc_pre = ADCPrescaler::DIV8;
    }

    let p = embassy_stm32::init(config);
    info!(
        "{} {} running",
        env!("CARGO_PKG_NAME"),
        env!("CARGO_PKG_VERSION")
    );

    let executor = EXECUTOR.init(Executor::new());
    executor.run(|spawner| {
        defmt::unwrap!(spawner.spawn(task::button_poller(p.PA9, p.PA8)));
        defmt::unwrap!(spawner.spawn(task::main_task(
            p.TIM2, p.TIM3, p.PA1, p.PA2, p.PA3, p.PA6, p.PA7, p.PB0
        )));
        defmt::unwrap!(spawner.spawn(task::detect_cc(p.PA4, p.PA5, p.ADC1)));
    });
}
