//use embassy_time::{Duration, Ticker, Timer};

use crate::task::{MainTaskMessage, MAIN_TASK_MESSAGES};

#[embassy_executor::task]
pub async fn dummy_pulser() {
        MAIN_TASK_MESSAGES.try_send(MainTaskMessage::DisableDummyLoad).ok();

//     let mut ticker = Ticker::every(Duration::from_secs(15));

//     loop {
//         ticker.next().await;
//         MAIN_TASK_MESSAGES.try_send(MainTaskMessage::EnableDummyLoad).ok();

//         Timer::after(Duration::from_millis(2000)).await;
//         //MAIN_TASK_MESSAGES.send(MainTaskMessage::DisableDummyLoad).await;
//     }
}
