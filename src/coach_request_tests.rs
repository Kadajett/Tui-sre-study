use super::*;
use std::{
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

fn reply(content: Value) -> String {
    json!({"choices":[{"finish_reason":"stop","message":{"content":content}}]}).to_string()
}

fn valid() -> Value {
    json!({"message":"The saved command reports allocated blocks.","assessment":"continue","evidence":"","practice_offered":false,"review_prompt_for":null,"review_result":null})
}

fn exchange(responses: Vec<String>) -> (Result<Message>, Vec<Value>) {
    let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
    let url = format!("http://{}", server.server_addr());
    let captured = Arc::new(Mutex::new(Vec::new()));
    let requests = captured.clone();
    let worker = thread::spawn(move || {
        for response in responses {
            let Some(mut request) = server.recv_timeout(Duration::from_secs(1)).unwrap() else {
                break;
            };
            let body: Value = serde_json::from_reader(request.as_reader()).unwrap();
            requests.lock().unwrap().push(body);
            request
                .respond(tiny_http::Response::from_string(response))
                .unwrap();
        }
    });
    let client = Client::new();
    let transport = Transport {
        client: &client,
        key: "test-only",
        url: &url,
    };
    let result = request_at(
        &transport,
        &[json!({"role":"user","content":"Explain saved output: 4 ./logs; exit 0"})],
        RequestOptions {
            teaching: true,
            tools: false,
            budget: &Budget::default(),
        },
    );
    worker.join().unwrap();
    let requests = captured.lock().unwrap().clone();
    (result, requests)
}

#[test]
fn empty_provider_reply_retries_saved_evidence_and_returns_the_answer() {
    let (result, requests) = exchange(vec![reply(Value::Null), reply(json!(valid().to_string()))]);
    let response = result.unwrap();
    assert!(crate::teaching_protocol::TeachingReply::parse(
        response.content.as_deref().unwrap_or("")
    )
    .is_ok());
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0]["messages"], requests[1]["messages"]);
    let format = &requests[0]["response_format"];
    assert_eq!(format["type"], "json_schema");
    assert_eq!(format["json_schema"]["strict"], true);
    assert_eq!(
        format["json_schema"]["schema"]["properties"]["message"]["minLength"],
        1
    );
}

#[test]
fn invalid_teaching_json_retries_before_the_learner_sees_an_error() {
    let (result, requests) = exchange(vec![reply(json!("{}")), reply(json!(valid().to_string()))]);
    let response = result.unwrap();
    assert!(crate::teaching_protocol::TeachingReply::parse(
        response.content.as_deref().unwrap_or("")
    )
    .is_ok());
    assert_eq!(requests.len(), 2);
}

#[test]
fn empty_replies_stop_after_three_attempts_with_an_actionable_error() {
    let (result, requests) = exchange(vec![reply(Value::Null); 3]);
    assert!(result.err().unwrap().to_string().contains("3 attempts"));
    assert_eq!(requests.len(), 3);
}

#[test]
fn tool_calls_are_not_mistaken_for_empty_answers_or_repeated() {
    let tool = json!({"choices":[{"message":{"content":null,"tool_calls":[{"id":"lookup-1","function":{"name":"list_docs","arguments":"{\"query\":\"du\"}"}}]}}]}).to_string();
    let (result, requests) = exchange(vec![tool]);
    assert_eq!(result.unwrap().tool_calls.len(), 1);
    assert_eq!(requests.len(), 1);
}

#[test]
fn exhausted_budget_does_not_start_another_http_request() {
    let client = Client::new();
    let transport = Transport {
        client: &client,
        key: "test-only",
        url: "http://127.0.0.1:1",
    };
    let budget = Budget::expired();
    let result = request_at(
        &transport,
        &[],
        RequestOptions {
            teaching: true,
            tools: false,
            budget: &budget,
        },
    );
    assert!(result
        .err()
        .unwrap()
        .to_string()
        .contains("60-second limit"));
}
