use embassy_futures::select::{Either, select};
use embassy_stm32::{
    Peri,
    gpio::{Level, Output, OutputType, Speed},
    time::khz,
    timer::{
        low_level::{CountingMode, OutputPolarity},
        simple_pwm::{PwmPin, SimplePwmChannels},
    },
};
use embassy_stm32::{
    peripherals::{ADC1, PA1, PA2, PA3, PA4, PA5, PA6, PA7, PB0, TIM2, TIM3},
    timer::simple_pwm::SimplePwm,
};
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, channel::Channel};
use embassy_time::{Duration, Timer};

pub static MAIN_TASK_MESSAGES: Channel<CriticalSectionRawMutex, MainTaskMessage, 4> =
    Channel::new();

#[derive(defmt::Format)]
pub enum MainTaskMessage {
    PlusButtonPressed,
    MinusButtonPressed,
    EnableDummyLoad,
    DisableDummyLoad,
    /// Initially, the load cannot be enabled. Only when enough power
    /// is detected to be available (via the CC lines of the USB Type-C connector),
    /// is this enabled. (It can also later be disabled again.)
    SetLoadLockedOut(bool),
}

#[derive(Clone, Copy)]
struct U16Fraction {
    pub numerator: u16,
    pub denominator: u16,
}

impl U16Fraction {
    const fn new(numerator: u16, denominator: u16) -> Self {
        U16Fraction {
            numerator,
            denominator,
        }
    }

    const fn mul(self, other: U16Fraction) -> Self {
        U16Fraction {
            numerator: self.numerator * other.numerator,
            denominator: self.denominator * other.denominator,
        }
    }
}

#[derive(Clone, Copy)]
struct State {
    pub fan: U16Fraction,
    pub dummy: U16Fraction,
    pub r: U16Fraction,
    pub g: U16Fraction,
    pub b: U16Fraction,
}

impl State {
    pub const fn new(fan_pct: u16, dummy_pct: u16, r: u16, g: u16, b: u16) -> Self {
        State {
            fan: U16Fraction::new(fan_pct, 100),
            dummy: U16Fraction::new(dummy_pct, 100),
            r: U16Fraction::new(r, 255),
            g: U16Fraction::new(g, 255),
            b: U16Fraction::new(b, 255),
        }
    }

    pub const fn with_brightness(self, factor: U16Fraction) -> Self {
        State {
            fan: self.fan,
            dummy: self.dummy,
            r: self.r.mul(factor),
            g: self.g.mul(factor),
            b: self.b.mul(factor),
        }
    }
}

static LED_BRIGHTNESS: U16Fraction = U16Fraction::new(2, 10);
const STATES: [State; 11] = [
    State::new(5, 0, 255, 0, 0).with_brightness(LED_BRIGHTNESS), // red
    State::new(10, 0, 255, 40, 0).with_brightness(LED_BRIGHTNESS), // orange?
    State::new(20, 0, 255, 127, 0).with_brightness(LED_BRIGHTNESS), // yellow
    State::new(30, 0, 160, 255, 0).with_brightness(LED_BRIGHTNESS), // light green
    State::new(40, 0, 0, 255, 0).with_brightness(LED_BRIGHTNESS),  // deep green
    State::new(50, 0, 90, 0, 255).with_brightness(LED_BRIGHTNESS), // violet
    State::new(60, 0, 0, 255, 255).with_brightness(LED_BRIGHTNESS), // teal
    State::new(70, 0, 0, 0, 255).with_brightness(LED_BRIGHTNESS),  // deep blue
    State::new(80, 0, 255, 40, 40).with_brightness(LED_BRIGHTNESS), // salmon
    State::new(90, 0, 255, 0, 255).with_brightness(LED_BRIGHTNESS), // pink
    State::new(100, 0, 255, 255, 255).with_brightness(LED_BRIGHTNESS), // white
];

static INITIAL_STATE_IDX: usize = 5;

static LED_ON_DURATION_AFTER_BUTTON_PRESS: Duration = Duration::from_secs(10);

#[embassy_executor::task]
pub async fn main_task(
    tim2: Peri<'static, TIM2>,
    tim3: Peri<'static, TIM3>,
    pa1: Peri<'static, PA1>,
    pa2: Peri<'static, PA2>,
    pa3: Peri<'static, PA3>,
    pa6: Peri<'static, PA6>,
    pa7: Peri<'static, PA7>,
    pb0: Peri<'static, PB0>,
) -> ! {
    let r_pin = PwmPin::new(pa1, OutputType::OpenDrain);
    let g_pin = PwmPin::new(pa2, OutputType::OpenDrain);
    let b_pin = PwmPin::new(pa3, OutputType::OpenDrain);

    let tim2_pwm = SimplePwm::new(
        tim2,
        None,
        Some(r_pin),
        Some(g_pin),
        Some(b_pin),
        khz(1),
        CountingMode::default(),
    );

    let SimplePwmChannels {
        ch1: _,
        ch2: mut r,
        ch3: mut g,
        ch4: mut b,
    } = tim2_pwm.split();

    r.set_polarity(OutputPolarity::ActiveLow);
    g.set_polarity(OutputPolarity::ActiveLow);
    b.set_polarity(OutputPolarity::ActiveLow);

    r.set_duty_cycle_fully_off();
    g.set_duty_cycle_fully_off();
    b.set_duty_cycle_fully_off();

    r.enable();
    g.enable();
    b.enable();

    let fan_pwm_pin = PwmPin::new(pa6, OutputType::PushPull);
    let dummy_load_pwm_pin = PwmPin::new(pa7, OutputType::PushPull);

    let tim3_pwm = SimplePwm::new(
        tim3,
        Some(fan_pwm_pin),
        Some(dummy_load_pwm_pin),
        None,
        None,
        khz(25),
        CountingMode::default(),
    );

    let SimplePwmChannels {
        ch1: mut fan,
        ch2: mut dummy_load,
        ch3: _,
        ch4: _,
    } = tim3_pwm.split();

    let mut load_enable = Output::new(pb0, Level::High, Speed::Low);

    fan.set_duty_cycle_fully_off();
    dummy_load.set_duty_cycle_fully_off();

    fan.enable();
    dummy_load.enable();

    let mut state_idx: usize = INITIAL_STATE_IDX;
    let mut led_turn_off_timer: Option<Timer> =
        Some(Timer::after(LED_ON_DURATION_AFTER_BUTTON_PRESS));
    let mut dummy_enabled = false;
    let mut load_locked_out = true;

    loop {
        if !load_locked_out {
            load_enable.set_high();

            let current_state = STATES[state_idx];
            defmt::info!(
                "Now on state {} ({}% fan, {}% dummy, dummy enabled: {})",
                state_idx,
                100 * current_state.fan.numerator / current_state.fan.denominator,
                100 * current_state.dummy.numerator / current_state.dummy.denominator,
                dummy_enabled
            );

            fan.set_duty_cycle_fraction(current_state.fan.numerator, current_state.fan.denominator);
            if dummy_enabled {
                dummy_load.set_duty_cycle_fraction(
                    current_state.dummy.numerator,
                    current_state.dummy.denominator,
                );
            } else {
                dummy_load.set_duty_cycle_fully_off();
            }
            if led_turn_off_timer.is_some() {
                r.set_duty_cycle_fraction(current_state.r.numerator, current_state.r.denominator);
                g.set_duty_cycle_fraction(current_state.g.numerator, current_state.g.denominator);
                b.set_duty_cycle_fraction(current_state.b.numerator, current_state.b.denominator);
            } else {
                r.set_duty_cycle_fully_off();
                g.set_duty_cycle_fully_off();
                b.set_duty_cycle_fully_off();
            }
        } else {
            // Load is locked out.
            fan.set_duty_cycle_fully_off();
            dummy_load.set_duty_cycle_fully_off();

            r.set_duty_cycle_fraction(1, 50);
            g.set_duty_cycle_fully_off();
            b.set_duty_cycle_fully_off();

            load_enable.set_low();
        }

        let event = if let Some(timer) = &mut led_turn_off_timer {
            select(MAIN_TASK_MESSAGES.receive(), timer).await
        } else {
            Either::First(MAIN_TASK_MESSAGES.receive().await)
        };

        match event {
            Either::First(MainTaskMessage::PlusButtonPressed) => {
                defmt::info!("Plus button was pressed");
                if (state_idx + 1) < STATES.len() {
                    state_idx += 1;
                }
                led_turn_off_timer = Some(Timer::after(LED_ON_DURATION_AFTER_BUTTON_PRESS));
            }
            Either::First(MainTaskMessage::MinusButtonPressed) => {
                defmt::info!("Minus button was pressed");
                if state_idx >= 1 {
                    state_idx -= 1;
                }
                led_turn_off_timer = Some(Timer::after(LED_ON_DURATION_AFTER_BUTTON_PRESS));
            }
            Either::First(MainTaskMessage::EnableDummyLoad) => {
                defmt::debug!("Enabling dummy load");
                dummy_enabled = true;
            }
            Either::First(MainTaskMessage::DisableDummyLoad) => {
                defmt::debug!("Disabling dummy load");
                dummy_enabled = false;
            }
            Either::First(MainTaskMessage::SetLoadLockedOut(locked_out)) => {
                if locked_out {
                    defmt::warn!(
                        "Locking out the load since available USB power has been decreased!"
                    )
                } else {
                    defmt::info!("Enabling load - enough USB power is available.")
                }
                load_locked_out = locked_out;
            }
            Either::Second(()) => {
                defmt::info!("Turning off the LED");
                // clear timer
                led_turn_off_timer = None;
            }
        }
    }
}
