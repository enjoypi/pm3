mod controller;
mod dto;
mod routes;

pub use self::{
    dto::{HEALTH_OK, HealthDto, StartRequestDto},
    routes::{APPS_PATH, HEALTH_PATH, router},
};
