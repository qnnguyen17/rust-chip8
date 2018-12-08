use scheduled_thread_pool::*;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

const NUM_WORKER_THREADS: usize = 2;
const ZERO_DURATION: Duration = Duration::from_secs(0);
const SIXTY_HZ_DURATION: Duration = Duration::from_millis(1000 / 60);

/// This struct handles starting and stopping the delay and sound timers.
pub struct Timers {
    delay_timer: Arc<Mutex<u8>>,
    delay_timer_handle: Option<JobHandle>,
    scheduler: ScheduledThreadPool,
}

impl Timers {
    pub fn new(delay_timer: Arc<Mutex<u8>>) -> Timers {
        let scheduler = ScheduledThreadPool::new(NUM_WORKER_THREADS);
        Timers {
            delay_timer,
            delay_timer_handle: Option::None,
            scheduler,
        }
    }

    pub fn start(&mut self) {
        let delay_timer = self.delay_timer.clone();
        let handle =
            self.scheduler
                .execute_at_fixed_rate(ZERO_DURATION, SIXTY_HZ_DURATION, move || {
                    let mut delay_timer = delay_timer.lock().unwrap();
                    if *delay_timer > 0 {
                        *delay_timer -= 1;
                    }
                });
        self.delay_timer_handle = Option::Some(handle)
    }

    pub fn stop(&self) {
        if let Some(handle) = &self.delay_timer_handle {
            handle.cancel();
        }
    }
}
