pub mod resolver;

#[cfg(test)]
mod tests {
    use super::resolver::{resolve, Command};

    #[test]
    fn test_ping_intent() {
        let cmd = resolve("Check status please").unwrap();
        assert_eq!(cmd, Command { intent: "PING".to_string(), target: "ALL".to_string() });
    }

    #[test]
    fn test_isolate_intent() {
        let cmd = resolve("Isolate the cluster").unwrap();
        assert_eq!(cmd, Command { intent: "ISOLATE".to_string(), target: "ALL".to_string() });
    }

    #[test]
    fn test_unknown_intent() {
        let res = resolve("Give me a coffee");
        assert!(res.is_err());
        assert_eq!(res.unwrap_err().error, "Unknown Intent");
    }
}
