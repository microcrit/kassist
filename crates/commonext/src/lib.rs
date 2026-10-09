use std::fmt::{Display, Formatter};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidationError {
    field: &'static str,
    message: String,
}

impl ValidationError {
    pub fn new(field: &'static str, message: impl Into<String>) -> Self {
        Self {
            field,
            message: message.into(),
        }
    }

    pub fn field(&self) -> &'static str {
        self.field
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl Display for ValidationError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.field, self.message)
    }
}

impl std::error::Error for ValidationError {}

#[macro_export]
macro_rules! ensure {
    ($condition:expr, $field:expr, $message:expr $(,)?) => {
        if !$condition {
            return Err($crate::ValidationError::new($field, $message));
        }
    };
}

#[cfg(test)]
mod tests {
    use super::ValidationError;

    #[test]
    fn error_includes_field_and_message() {
        let error = ValidationError::new("plugin_id", "must not be empty");
        assert_eq!(error.to_string(), "plugin_id: must not be empty");
        assert_eq!(error.field(), "plugin_id");
        assert_eq!(error.message(), "must not be empty");
    }

    #[test]
    fn ensure_returns_validation_error() {
        fn only_positive(value: i32) -> Result<(), ValidationError> {
            crate::ensure!(value > 0, "value", "must be positive");
            Ok(())
        }

        assert!(only_positive(1).is_ok());
        assert_eq!(only_positive(0).unwrap_err().field(), "value");
    }
}
