mod controller;
mod dto;
mod routes;

pub use self::{
    dto::{HEALTH_OK, HealthDto, ReplyDto, StartRequestDto},
    routes::{APPS_PATH, HEALTH_PATH, SERVICES_STOP_ALL_PATH, router},
};
