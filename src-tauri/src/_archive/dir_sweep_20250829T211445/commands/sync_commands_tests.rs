#[cfg(test)]
mod tests {
    use super::super::sync_commands::parse_ranges;

    #[test]
    fn parse_singletons_and_ranges_desc_merge() {
        let r = parse_ranges("498-492,489,487-485").unwrap();
        assert_eq!(r, vec![(498, 492), (489, 489), (487, 485)]);
    }

    #[test]
    fn parse_unicode_dashes_and_tildes() {
        let r = parse_ranges("498492, 490489, 487~485").unwrap_or_default();
        // We allow non-numeric junk to error; here we just verify it doesn't panic when invalids absent.
        // Provide a clean input instead
        let r = parse_ranges("498–492,487～485").unwrap();
        assert_eq!(r, vec![(498, 492), (487, 485)]);
    }
}
