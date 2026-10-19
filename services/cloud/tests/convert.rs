//! canvist Convert: formats, limits, counting, and roles.

mod support;

use canvist_cloud::auth::Scope;
use canvist_cloud::config::Role;
use serde_json::Value as Json;
use serde_json::json;
use support::Fixture;
use support::deno::Client;
use support::deno::Transport;

async fn convert(fixture: &Fixture, request: &Json) -> (u16, Json) {
	let response = fixture
		.api()
		.post("/v1/convert")
		.json(request)
		.send()
		.await
		.expect("responds");
	let status = response.status().as_u16();
	(status, response.json().await.unwrap_or(Json::Null))
}

#[tokio::test(flavor = "multi_thread")]
async fn markdown_html_json_and_text_convert_into_each_other() {
	let fixture = Fixture::start().await;
	let (status, html) = convert(
		&fixture,
		&json!({ "from": "markdown", "to": "html", "input": "# Plan\n\n- ship **it**" }),
	)
	.await;
	assert_eq!(status, 200);
	assert_eq!(html["format"], "html");
	assert_eq!(
		html["output"],
		"<h1>Plan</h1><ul><li><p>ship <strong>it</strong></p></li></ul>"
	);

	let (_, document) = convert(
		&fixture,
		&json!({ "from": "html", "to": "json", "input": html["output"] }),
	)
	.await;
	assert_eq!(
		document["output"]["type"], "doc",
		"JSON output is an object"
	);

	let (_, markdown) = convert(
		&fixture,
		&json!({ "from": "json", "to": "markdown", "input": document["output"] }),
	)
	.await;
	assert_eq!(markdown["output"], "# Plan\n\n- ship **it**");

	let (_, text) = convert(
		&fixture,
		&json!({ "from": "markdown", "to": "text", "input": "# Plan\n\nship it" }),
	)
	.await;
	assert_eq!(text["output"], "Plan\nship it");
}

#[tokio::test(flavor = "multi_thread")]
async fn bad_requests_are_explained() {
	let mut config = support::config();
	config.convert.max_input_bytes = 1024;
	let fixture = Fixture::start_with(config, support::store().await).await;

	let (status, body) = convert(
		&fixture,
		&json!({ "from": "docx", "to": "html", "input": "" }),
	)
	.await;
	assert_eq!(
		(status, body["error"]["code"].as_str()),
		(400, Some("bad_request"))
	);
	let (status, _) = convert(
		&fixture,
		&json!({ "from": "markdown", "to": "html", "input": 42 }),
	)
	.await;
	assert_eq!(status, 400);
	let (status, body) = convert(
		&fixture,
		&json!({ "from": "json", "to": "html", "input": "{ not json" }),
	)
	.await;
	assert_eq!(
		(status, body["error"]["code"].as_str()),
		(422, Some("unconvertible"))
	);
	let (status, _) = convert(
		&fixture,
		&json!({ "from": "text", "to": "html", "input": "x".repeat(2048) }),
	)
	.await;
	assert_eq!(status, 413, "the per-request size limit applies");

	let anonymous = reqwest::Client::new()
		.post(fixture.http("/v1/convert"))
		.json(&json!({ "from": "text", "to": "html", "input": "x" }))
		.send()
		.await
		.expect("responds");
	assert_eq!(anonymous.status(), 401);
}

#[tokio::test(flavor = "multi_thread")]
async fn conversions_count_toward_usage() {
	let fixture = Fixture::start().await;

	for _ in 0..3 {
		convert(
			&fixture,
			&json!({ "from": "text", "to": "html", "input": "hello" }),
		)
		.await;
	}

	let usage: Json = fixture
		.api()
		.get("/v1/usage")
		.send()
		.await
		.expect("responds")
		.json()
		.await
		.expect("json");
	assert_eq!(usage["environments"][0]["conversions"], 3);
	assert_eq!(
		usage["active_documents"], 0,
		"conversions are not active documents"
	);
}

#[tokio::test(flavor = "multi_thread")]
async fn synced_documents_export_as_markdown() {
	let fixture = Fixture::start().await;
	let mut client = Client::spawn();
	let token = fixture.token("notes", Scope::Write);
	assert!(
		client
			.connect(Transport::YWebsocket, &fixture.sync_url(), "notes", &token)
			.synced
	);
	client.append("Typed in a browser");
	client.wait_texts(&["Typed in a browser"]);
	tokio::time::sleep(std::time::Duration::from_millis(100)).await;

	let exported: Json = fixture
		.api()
		.get("/v1/documents/notes?format=markdown")
		.send()
		.await
		.expect("responds")
		.json()
		.await
		.expect("json");
	assert_eq!(
		exported,
		json!({ "format": "markdown", "output": "Typed in a browser" })
	);

	let unknown = fixture
		.api()
		.get("/v1/documents/notes?format=pdf")
		.send()
		.await
		.expect("responds");
	assert_eq!(unknown.status(), 400);
}

#[tokio::test(flavor = "multi_thread")]
async fn roles_serve_only_their_routes() {
	let mut config = support::config();
	config.role = Role::Convert;
	let convert_only = Fixture::start_with(config, support::store().await).await;
	let (status, _) = convert(
		&convert_only,
		&json!({ "from": "text", "to": "html", "input": "x" }),
	)
	.await;
	assert_eq!(status, 200);
	let documents = convert_only
		.api()
		.get("/v1/documents")
		.send()
		.await
		.expect("responds");
	assert_eq!(documents.status(), 404);

	let mut config = support::config();
	config.role = Role::Sync;
	let sync_only = Fixture::start_with(config, support::store().await).await;
	let (status, _) = convert(
		&sync_only,
		&json!({ "from": "text", "to": "html", "input": "x" }),
	)
	.await;
	assert_eq!(status, 404);
}
