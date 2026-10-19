import { createEditor } from "../../src/mod.ts";

const markdown = new URLSearchParams(location.search).get("md") ??
	"Every pixel here is painted on a **canvas**, measured by the browser, so it wraps exactly like the *server-rendered* HTML. AVATAR Wa To ff fi fl.\n\n- one `code`\n- [link](https://x.y) ~~strike~~";
const element = document.querySelector<HTMLElement>("#canvas")!;
const editor = await createEditor(element, {
	content: { format: "markdown", input: markdown },
});
await editor.ready();
const { html, css } = editor.toHtml();
const style = document.createElement("style");
style.textContent = css;
document.head.append(style);

document.querySelector("#dom")!.innerHTML = html;
Object.assign(globalThis, { editor });
