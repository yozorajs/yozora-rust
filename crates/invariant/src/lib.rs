const PREFIX: &str = "Invariant failed";

pub fn invariant(condition: bool, message: Option<&str>) {
    if condition {
        return;
    }

    if std::env::var("NODE_ENV").as_deref() == Ok("production") {
        panic!("{PREFIX}");
    }

    panic!("{PREFIX}: {}", message.unwrap_or_default());
}

#[cfg(test)]
mod tests {
    use super::invariant;

    #[test]
    fn accepts_true_condition() {
        invariant(true, Some("unused"));
    }

    #[test]
    #[should_panic(expected = "Invariant failed: context")]
    fn reports_context_in_non_production_mode() {
        invariant(false, Some("context"));
    }
}
