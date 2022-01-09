use std::cell::RefCell;
use std::sync::{Arc, Mutex};

use once_cell::sync::Lazy;

use super::schedule::Scheduler;
use super::{Object};

static BALANCER: Lazy<Mutex<Balancer>> = Lazy::new(|| {
    let balancer = Balancer {
        scheduler: Scheduler::new(),
    };

    Mutex::new(balancer)
});


struct Balancer {
    scheduler: Scheduler,
}

pub fn add_object(obj: Arc<RefCell<Object>>) {
    let balancer = BALANCER.lock().unwrap();
    balancer.scheduler.add_object(obj);
}

