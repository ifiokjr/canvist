# canvist Cloud Convert

In this guide you convert documents between formats with one HTTP call, using the same schema the editor uses. Use it when you'd rather not run canvist yourself, for example from a language canvist has no package for.

## Convert a document

Send `POST /v1/convert` to your project's Convert host with your API key, the two formats, and the input:

```ts no_run
const response = await fetch("https://CONVERT_HOST/v1/convert", {
	method: "POST",
	headers: {
		authorization: `Bearer ${Deno.env.get("CANVIST_API_KEY")}`,
		"content-type": "application/json",
	},
	body: JSON.stringify({
		from: "markdown",
		to: "html",
		input: "# Plan\n\n- ship **it**",
	}),
});
const { output } = await response.json();
// <h1>Plan</h1><ul><li><p>ship <strong>it</strong></p></li></ul>
```

```sh
curl -X POST https://CONVERT_HOST/v1/convert \
  -H "Authorization: Bearer $CANVIST_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{ "from": "markdown", "to": "json", "input": "Hello **world**" }'
```

- Formats are `json` (ProseMirror and TipTap JSON), `html`, `markdown`, and `text`.
- `input` is a string. For `json` it can also be an object.
- `output` is an object for `json`, and a string otherwise.

Conversion uses the StarterKit schema, the same rules as [Import and export](../guides/import-export.md): JSON must fit the schema, and HTML and Markdown never fail.

## Convert a stored document

A document stored in canvist Cloud Sync converts on read, from your Sync host:

```sh
curl "https://SYNC_HOST/v1/documents/team%2Fnotes?format=markdown" \
  -H "Authorization: Bearer $CANVIST_API_KEY"
```

## Limits and errors

| Status | When                                           |
| ------ | ---------------------------------------------- |
| `400`  | An unknown format, or the wrong input type     |
| `413`  | The request is over 8 MiB                      |
| `422`  | The input doesn't parse (code `unconvertible`) |
| `504`  | The conversion took longer than 10 seconds     |

A 10,000-paragraph document converts in 15 to 50 ms. Every conversion counts toward the month's usage; see [Plans and billing](./billing.md).

## Run it yourself instead

The same conversions run in-process with the [Rust](../quick-start/server-rust.md), [TypeScript](../quick-start/server-typescript.md), and [Dart](../quick-start/server-dart.md) packages, with no network call and no usage.
