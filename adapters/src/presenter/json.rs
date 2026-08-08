use crate::http::ProcessViewDto;

#[must_use]
pub fn render_json_list(views: &[ProcessViewDto]) -> String {
    serde_json::to_string(views)
        .expect("internal error: serializing a process view list cannot fail")
}

#[must_use]
pub fn render_json_one(view: Option<&ProcessViewDto>) -> String {
    serde_json::to_string(&view).expect("internal error: serializing a process view cannot fail")
}

#[cfg(test)]
#[path = "../tests/presenter_json_tests.rs"]
mod tests;
