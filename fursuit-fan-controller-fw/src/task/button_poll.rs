use debouncr::{Edge, debounce_stateful_8};
use embassy_stm32::{
    Peri,
    gpio::{Input, Pull},
    peripherals::{PA8, PA9},
};
use embassy_time::{Duration, Ticker};

use crate::task::{MAIN_TASK_MESSAGES, MainTaskMessage};

#[embassy_executor::task]
pub async fn button_poller(pa9: Peri<'static, PA9>, pa8: Peri<'static, PA8>) -> ! {
    let plus_btn = Input::new(pa9, Pull::Up);
    let minus_btn = Input::new(pa8, Pull::Up);

    let mut plus_debouncer = debounce_stateful_8(plus_btn.is_low());
    let mut minus_debouncer = debounce_stateful_8(minus_btn.is_low());

    let mut ticker = Ticker::every(Duration::from_millis(5));
    loop {
        ticker.next().await;

        if plus_debouncer.update(plus_btn.is_low()) == Some(Edge::Rising) {
            MAIN_TASK_MESSAGES
                .try_send(MainTaskMessage::PlusButtonPressed)
                .ok();
        }

        if minus_debouncer.update(minus_btn.is_low()) == Some(Edge::Rising) {
            MAIN_TASK_MESSAGES
                .try_send(MainTaskMessage::MinusButtonPressed)
                .ok();
        }
    }
}
