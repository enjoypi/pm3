use std::fmt::Write as _;

use thiserror::Error;

const FIELD_COUNT: usize = 5;
const FIELD_BOUNDS: [(u32, u32); FIELD_COUNT] = [(0, 59), (0, 23), (1, 31), (1, 12), (0, 6)];
const RANDOM_MARK: char = '~';
const STEP_MARK: char = '/';

#[derive(Debug, Eq, PartialEq, Error)]
pub enum ExpandError {
    #[error("cannot accept step 0 in random field '{field}'")]
    ZeroStep { field: String },

    #[error("cannot accept random field '{field}': the low bound exceeds the high bound")]
    InvertedRange { field: String },

    #[error("cannot accept random field '{field}': bounds must fall within {low}-{high}")]
    OutOfRange { field: String, low: u32, high: u32 },

    #[error("cannot parse random field '{field}': expected '~', 'low~high' or 'low~high/step'")]
    Malformed { field: String },
}

pub fn expand_random(pattern: &str, rng: &mut fastrand::Rng) -> Result<String, ExpandError> {
    let fields: Vec<&str> = pattern.split_whitespace().collect();
    if fields.len() != FIELD_COUNT {
        return Ok(pattern.to_string());
    }

    let mut expanded = String::with_capacity(pattern.len());
    for (index, field) in fields.iter().enumerate() {
        let bounds = FIELD_BOUNDS[index];
        let rendered = expand_field(field, bounds, rng)?;
        let separator = if index == 0 { "" } else { " " };
        let _ = write!(expanded, "{separator}{rendered}");
    }
    Ok(expanded)
}

fn expand_field(
    field: &str,
    bounds: (u32, u32),
    rng: &mut fastrand::Rng,
) -> Result<String, ExpandError> {
    let Some((low_text, tail)) = field.split_once(RANDOM_MARK) else {
        return Ok(field.to_string());
    };
    let (high_text, step) = split_step(field, tail)?;
    let (low, high) = parse_bounds(field, low_text, high_text, bounds)?;

    let Some(step) = step else {
        return Ok(rng.u32(low..=high).to_string());
    };
    let offset = low.saturating_add(rng.u32(0..step));
    Ok(format!("{}-{high}/{step}", offset.min(high)))
}

fn split_step<'t>(field: &str, tail: &'t str) -> Result<(&'t str, Option<u32>), ExpandError> {
    let Some((high, step)) = tail.split_once(STEP_MARK) else {
        return Ok((tail, None));
    };
    let step = step
        .parse::<u32>()
        .map_err(|_parse| ExpandError::Malformed {
            field: field.to_string(),
        })?;
    if step == 0 {
        return Err(ExpandError::ZeroStep {
            field: field.to_string(),
        });
    }
    Ok((high, Some(step)))
}

fn parse_bounds(
    field: &str,
    low_text: &str,
    high_text: &str,
    bounds: (u32, u32),
) -> Result<(u32, u32), ExpandError> {
    let (low_bound, high_bound) = bounds;
    let low = parse_bound(field, low_text, low_bound)?;
    let high = parse_bound(field, high_text, high_bound)?;
    if low > high {
        return Err(ExpandError::InvertedRange {
            field: field.to_string(),
        });
    }
    if low < low_bound || high > high_bound {
        return Err(ExpandError::OutOfRange {
            field: field.to_string(),
            low: low_bound,
            high: high_bound,
        });
    }
    Ok((low, high))
}

fn parse_bound(field: &str, text: &str, fallback: u32) -> Result<u32, ExpandError> {
    if text.is_empty() {
        return Ok(fallback);
    }
    text.parse::<u32>()
        .map_err(|_parse| ExpandError::Malformed {
            field: field.to_string(),
        })
}

#[cfg(test)]
#[path = "../tests/schedule_random_expand_tests.rs"]
mod tests;
