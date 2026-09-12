use super::*;

#[test]
fn operator_configured_reference_hosts_are_portable() {
    for url in [
        "https://docs.example.com",
        "https://search.example.net/personal",
        "https://docs.example-tailnet.ts.net",
        "http://searxng:8080",
        "http://127.0.0.1:9292",
    ] {
        assert_eq!(parse_base(url).unwrap(), url);
    }
    assert_eq!(
        parse_base("https://docs.example.com/").unwrap(),
        "https://docs.example.com"
    );
}

#[test]
fn reference_urls_reject_credentials_and_non_http_schemes() {
    for url in [
        "",
        "file:///etc/passwd",
        "ftp://example.com",
        "https://user:secret@example.com",
        "https://example.com?token=secret",
        "https://example.com/#fragment",
    ] {
        assert!(parse_base(url).is_err(), "Accepted {url}");
    }
}

#[test]
fn only_configured_reference_tools_are_offered() {
    let definitions = json!([
        {"function":{"name":"list_docs"}},
        {"function":{"name":"search_docs"}},
        {"function":{"name":"read_doc"}},
        {"function":{"name":"search_web"}}
    ]);
    assert!(selected_definitions(definitions.clone(), false, false).is_empty());
    assert_eq!(
        selected_definitions(definitions.clone(), true, false).len(),
        3
    );
    let search = selected_definitions(definitions.clone(), false, true);
    assert_eq!(search.len(), 1);
    assert_eq!(search[0]["function"]["name"], "search_web");
    assert_eq!(selected_definitions(definitions, true, true).len(), 4);
}
