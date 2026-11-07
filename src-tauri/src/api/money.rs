use rust_decimal::{Decimal, RoundingStrategy};
#[cfg(test)]
use std::str::FromStr;

pub const SCALE: u32 = 3;

#[cfg(test)]
pub fn parse(value: impl AsRef<str>) -> Result<Decimal, String> {
    Decimal::from_str(value.as_ref())
        .map(|value| round(value))
        .map_err(|_| "Invalid decimal money value.".to_string())
}

pub fn round(value: Decimal) -> Decimal {
    value.round_dp_with_strategy(SCALE, RoundingStrategy::MidpointAwayFromZero)
}

pub fn normalize(value: Decimal) -> String {
    format!("{:.3}", round(value))
}

pub fn percentage(amount: Decimal, rate: Decimal) -> Decimal {
    round(amount * rate / Decimal::ONE_HUNDRED)
}

#[cfg(test)]
pub fn divide(amount: Decimal, divisor: u32) -> Result<Decimal, String> {
    if divisor == 0 {
        return Err("Money divisor must be positive.".to_string());
    }

    Ok(round(amount / Decimal::from(divisor)))
}

pub fn sum(values: impl IntoIterator<Item = Decimal>) -> Decimal {
    round(values.into_iter().sum())
}

#[cfg(test)]
mod tests {
    use super::{divide, normalize, parse, percentage};

    #[test]
    fn decimal_operations_match_the_legacy_backend() {
        assert_eq!(normalize(parse("80").unwrap()), "80.000");
        assert_eq!(
            normalize(parse("60.100").unwrap() + parse("40.025").unwrap()),
            "100.125"
        );
        assert_eq!(
            normalize(divide(parse("100").unwrap(), 3).unwrap()),
            "33.333"
        );
        assert_eq!(
            normalize(percentage(parse("80").unwrap(), parse("30").unwrap())),
            "24.000"
        );
    }
}
