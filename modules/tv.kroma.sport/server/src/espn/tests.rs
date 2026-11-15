use super::{scoreboard, standings, summary};

#[test]
fn the_urls_are_the_documented_public_endpoints() {
    assert_eq!(
        scoreboard("fra.1", "20261103"),
        "/apis/site/v2/sports/soccer/fra.1/scoreboard?dates=20261103"
    );
    assert_eq!(
        summary("all", "401"),
        "/apis/site/v2/sports/soccer/all/summary?event=401"
    );
    assert_eq!(standings("fra.1"), "/apis/v2/sports/soccer/fra.1/standings");
}

#[cfg(not(target_family = "wasm"))]
mod socket {
    use serde_json::json;

    use crate::espn::fake::FakeEspn;
    use crate::espn::{scoreboard, Espn, HttpEspn};

    #[test]
    fn reads_json_from_a_real_socket_and_refuses_a_failing_status() {
        let fake = FakeEspn::start();
        fake.serve(
            "/apis/site/v2/sports/soccer/fra.1/scoreboard?dates=20261103",
            &json!({"events": []}),
        );
        fake.fail("/broken", 500);
        let base = fake.base.clone();
        let client = HttpEspn::new(move || base.clone());

        let ok = client.get(&scoreboard("fra.1", "20261103")).unwrap();
        let missing = client.get("/nowhere").unwrap_err().to_string();
        let broken = client.get("/broken").unwrap_err().to_string();

        assert_eq!(ok, json!({"events": []}));
        assert!(missing.contains("404") && broken.contains("500"));
        assert_eq!(fake.calls().len(), 3);
    }
}
