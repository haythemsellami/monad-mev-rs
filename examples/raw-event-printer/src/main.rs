fn main() {
    let fixture =
        monad_mev_events::load_workspace_fixture("raw-events.json").expect("fixture should load");
    println!("{} events", fixture.events.len());
}

#[cfg(test)]
mod tests {
    #[test]
    fn raw_event_printer_fixture_loads() {
        let fixture = monad_mev_events::load_workspace_fixture("raw-events.json")
            .expect("fixture should load");

        assert_eq!(fixture.name, "raw-events");
    }
}
