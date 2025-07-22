//! Event handling for Inngest

pub use inngest_core::{Event, Result};

#[cfg(test)]
mod tests {
    
    use pretty_assertions::assert_eq;

    #[test]
    fn test_placeholder() {
        let fixture = true;
        let actual = fixture;
        let expected = true;
        assert_eq!(actual, expected);
    }
}
