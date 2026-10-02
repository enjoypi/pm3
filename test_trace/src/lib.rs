use std::fmt::Debug;

use tracing::{
    Event, Metadata, Subscriber,
    field::{Field, Visit},
    span::{Attributes, Id, Record},
};

struct TraceAll;

impl Subscriber for TraceAll {
    fn enabled(&self, _metadata: &Metadata<'_>) -> bool {
        true
    }

    fn new_span(&self, span: &Attributes<'_>) -> Id {
        span.record(&mut Render);
        Id::from_u64(1)
    }

    fn record(&self, _span: &Id, values: &Record<'_>) {
        values.record(&mut Render);
    }

    fn record_follows_from(&self, _span: &Id, _follows: &Id) {}

    fn event(&self, event: &Event<'_>) {
        event.record(&mut Render);
    }

    fn enter(&self, _span: &Id) {}

    fn exit(&self, _span: &Id) {}
}

struct Render;

impl Visit for Render {
    fn record_debug(&mut self, _field: &Field, value: &dyn Debug) {
        std::hint::black_box(format!("{value:?}"));
    }
}

pub fn install() {
    tracing::subscriber::set_global_default(TraceAll).ok();
}
